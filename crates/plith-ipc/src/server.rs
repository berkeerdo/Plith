//! The pipe server that runs inside the UIAccess OSD helper.
//!
//! Threading: [`OsdServer::serve`] runs on one thread and blocks it. A [`Stopper`] may
//! call [`Stopper::stop`] from any thread.

use std::mem::size_of;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{
    DUPLICATE_SAME_ACCESS, DuplicateHandle, ERROR_ACCESS_DENIED, ERROR_BROKEN_PIPE,
    ERROR_MORE_DATA, ERROR_NO_DATA, ERROR_OPERATION_ABORTED, ERROR_PIPE_BUSY, ERROR_PIPE_CONNECTED,
    ERROR_PIPE_NOT_CONNECTED, FALSE, GetLastError, HANDLE, HLOCAL, LocalFree,
};
use windows::Win32::Security::Authorization::{
    ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows::Win32::Security::{PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES};
use windows::Win32::Storage::FileSystem::{
    FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_INBOUND, ReadFile,
};
use windows::Win32::System::IO::CancelSynchronousIo;
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_READMODE_MESSAGE,
    PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_MESSAGE, PIPE_WAIT,
};
use windows::Win32::System::Threading::{GetCurrentProcess, GetCurrentThread, GetCurrentThreadId};
use windows::core::PCWSTR;

use crate::security::{pipe_sddl, token};
use crate::win::{OwnedHandle, wide};
use crate::{DecodeError, MAX_MESSAGE, Message, decode};

/// Why the server pipe could not be created.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BindError {
    /// Another server already owns the pipe name.
    AlreadyRunning,
    /// Any other Windows error, as text.
    Os(String),
}

impl std::fmt::Display for BindError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlreadyRunning => write!(f, "the OSD pipe is already served"),
            Self::Os(text) => write!(f, "{text}"),
        }
    }
}

impl std::error::Error for BindError {}

/// What the message callback wants the server to do next.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flow {
    /// Keep reading.
    Continue,
    /// Stop serving.
    Stop,
}

#[derive(Default)]
struct Shared {
    stop: AtomicBool,
    serving_thread: AtomicU32,
    /// A real handle to the serving thread, present only while `serve` runs.
    thread: Mutex<Option<OwnedHandle>>,
}

/// Stops a running [`OsdServer::serve`] from any thread.
#[derive(Clone)]
pub struct Stopper {
    shared: Arc<Shared>,
}

impl Stopper {
    /// Asks the server to stop and aborts its blocking pipe call.
    ///
    /// Returns within about one second even if the callback never returns.
    pub fn stop(&self) {
        self.shared.stop.store(true, Ordering::SeqCst);
        // SAFETY: plain query of the calling thread id.
        if self.shared.serving_thread.load(Ordering::SeqCst) == unsafe { GetCurrentThreadId() } {
            return;
        }
        let deadline = Instant::now() + Duration::from_secs(1);
        // The serving thread may not be inside a pipe call yet, so cancel until it leaves.
        while Instant::now() < deadline {
            let guard = self
                .shared
                .thread
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            let Some(handle) = guard.as_ref() else {
                return;
            };
            // SAFETY: the handle stays valid while the guard is held.
            unsafe {
                let _ = CancelSynchronousIo(handle.0);
            }
            drop(guard);
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

/// The named pipe server of the OSD helper.
pub struct OsdServer {
    pipe: OwnedHandle,
    shared: Arc<Shared>,
}

enum Read {
    Message(usize),
    TooBig,
    Closed,
    Aborted,
    Failed(String),
}

enum Pumped {
    Stop,
    Disconnect,
    Failed(String),
}

impl OsdServer {
    /// Creates the pipe `name` (for example from [`crate::current_pipe_name`]).
    ///
    /// The pipe is a single message-mode instance, refuses remote clients and only the
    /// current user may open it.
    ///
    /// # Errors
    /// [`BindError::AlreadyRunning`] when the name is taken, [`BindError::Os`] otherwise.
    pub fn bind(name: &str) -> Result<Self, BindError> {
        let sid = token::current_user_sid().map_err(BindError::Os)?;
        let sddl = wide(&pipe_sddl(&sid));
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        // SAFETY: `sddl` is NUL-terminated and `descriptor` is a valid out pointer.
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                PCWSTR(sddl.as_ptr()),
                SDDL_REVISION_1,
                &raw mut descriptor,
                None,
            )
        }
        .map_err(|e| BindError::Os(format!("security descriptor: {e}")))?;
        let attributes = SECURITY_ATTRIBUTES {
            nLength: u32::try_from(size_of::<SECURITY_ATTRIBUTES>()).unwrap_or(0),
            lpSecurityDescriptor: descriptor.0,
            bInheritHandle: FALSE,
        };
        let wide_name = wide(name);
        // SAFETY: the name and attributes outlive the call; the descriptor is freed after it.
        let handle = unsafe {
            CreateNamedPipeW(
                PCWSTR(wide_name.as_ptr()),
                PIPE_ACCESS_INBOUND | FILE_FLAG_FIRST_PIPE_INSTANCE,
                PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                1,
                0,
                4096,
                0,
                Some(&raw const attributes),
            )
        };
        // SAFETY: read the error before any other call can overwrite it.
        let error = unsafe { GetLastError() };
        // SAFETY: the descriptor was allocated by the conversion call and is freed once.
        unsafe {
            let _ = LocalFree(Some(HLOCAL(descriptor.0)));
        }
        if handle.is_invalid() {
            return Err(
                if error == ERROR_ACCESS_DENIED || error == ERROR_PIPE_BUSY {
                    BindError::AlreadyRunning
                } else {
                    BindError::Os(format!(
                        "CreateNamedPipeW: {}",
                        windows::core::Error::from(error)
                    ))
                },
            );
        }
        Ok(Self {
            pipe: OwnedHandle(handle),
            shared: Arc::new(Shared::default()),
        })
    }

    /// Returns a handle that stops [`serve`](Self::serve) from any thread.
    #[must_use]
    pub fn stopper(&self) -> Stopper {
        Stopper {
            shared: Arc::clone(&self.shared),
        }
    }

    /// Serves clients one after another and reports every message to `on`.
    ///
    /// Blocks the calling thread until `on` returns [`Flow::Stop`], the [`Stopper`] fires
    /// or a fatal error occurs. A client that sends a fatal decode error (bad magic or
    /// version) is disconnected and the next client is awaited.
    ///
    /// # Errors
    /// Returns the Windows error text on an unexpected pipe failure.
    pub fn serve(
        &self,
        mut on: impl FnMut(Result<Message, DecodeError>) -> Flow,
    ) -> Result<(), String> {
        let _registration = Registration::new(&self.shared)?;
        loop {
            if self.shared.stop.load(Ordering::SeqCst) {
                return Ok(());
            }
            if let Err(error) = self.connect() {
                return if self.shared.stop.load(Ordering::SeqCst) {
                    Ok(())
                } else {
                    Err(error)
                };
            }
            let outcome = self.pump(&mut on);
            // SAFETY: the pipe handle is valid; a failure only means nothing was connected.
            unsafe {
                let _ = DisconnectNamedPipe(self.pipe.0);
            }
            match outcome {
                Pumped::Stop => return Ok(()),
                Pumped::Disconnect => {}
                Pumped::Failed(error) => return Err(error),
            }
        }
    }

    fn connect(&self) -> Result<(), String> {
        // SAFETY: the pipe handle is valid and the call is synchronous.
        match unsafe { ConnectNamedPipe(self.pipe.0, None) } {
            Ok(()) => Ok(()),
            // The client connected, wrote and closed before this call; its data is still
            // readable, so carry on to the read loop.
            Err(e)
                if e.code() == ERROR_PIPE_CONNECTED.to_hresult()
                    || e.code() == ERROR_NO_DATA.to_hresult() =>
            {
                Ok(())
            }
            Err(e) => Err(format!("ConnectNamedPipe: {e}")),
        }
    }

    fn pump(&self, on: &mut impl FnMut(Result<Message, DecodeError>) -> Flow) -> Pumped {
        let mut buffer = [0u8; MAX_MESSAGE];
        loop {
            if self.shared.stop.load(Ordering::SeqCst) {
                return Pumped::Stop;
            }
            let (result, fatal) = match self.read_message(&mut buffer) {
                Read::Message(len) => {
                    let result = decode(&buffer[..len]);
                    let fatal = result.as_ref().err().is_some_and(DecodeError::is_fatal);
                    (result, fatal)
                }
                Read::TooBig => {
                    self.drain();
                    (Err(DecodeError::Trailing), false)
                }
                Read::Closed => return Pumped::Disconnect,
                Read::Aborted => continue,
                Read::Failed(error) => return Pumped::Failed(error),
            };
            if on(result) == Flow::Stop {
                return Pumped::Stop;
            }
            if fatal {
                return Pumped::Disconnect;
            }
        }
    }

    fn read_message(&self, buffer: &mut [u8]) -> Read {
        let mut read = 0u32;
        // SAFETY: the buffer and `read` are valid for the duration of the synchronous call.
        match unsafe { ReadFile(self.pipe.0, Some(buffer), Some(&raw mut read), None) } {
            Ok(()) => Read::Message(read as usize),
            Err(e) => classify(&e),
        }
    }

    /// Discards the rest of an oversized message without keeping it.
    fn drain(&self) {
        let mut scratch = [0u8; MAX_MESSAGE];
        while matches!(self.read_message(&mut scratch), Read::TooBig) {}
    }
}

fn classify(error: &windows::core::Error) -> Read {
    let code = error.code();
    if code == ERROR_MORE_DATA.to_hresult() {
        Read::TooBig
    } else if code == ERROR_BROKEN_PIPE.to_hresult()
        || code == ERROR_NO_DATA.to_hresult()
        || code == ERROR_PIPE_NOT_CONNECTED.to_hresult()
    {
        Read::Closed
    } else if code == ERROR_OPERATION_ABORTED.to_hresult() {
        Read::Aborted
    } else {
        Read::Failed(format!("ReadFile: {error}"))
    }
}

/// Publishes the serving thread to the [`Stopper`] while `serve` runs.
struct Registration<'a> {
    shared: &'a Shared,
}

impl<'a> Registration<'a> {
    fn new(shared: &'a Shared) -> Result<Self, String> {
        let mut duplicate = HANDLE::default();
        // SAFETY: duplicating the current thread pseudo handle into a real handle.
        unsafe {
            DuplicateHandle(
                GetCurrentProcess(),
                GetCurrentThread(),
                GetCurrentProcess(),
                &raw mut duplicate,
                0,
                false,
                DUPLICATE_SAME_ACCESS,
            )
        }
        .map_err(|e| format!("DuplicateHandle: {e}"))?;
        // SAFETY: plain query of the calling thread id.
        shared
            .serving_thread
            .store(unsafe { GetCurrentThreadId() }, Ordering::SeqCst);
        *shared.thread.lock().unwrap_or_else(PoisonError::into_inner) =
            Some(OwnedHandle(duplicate));
        Ok(Self { shared })
    }
}

impl Drop for Registration<'_> {
    fn drop(&mut self) {
        self.shared.serving_thread.store(0, Ordering::SeqCst);
        *self
            .shared
            .thread
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = None;
    }
}
