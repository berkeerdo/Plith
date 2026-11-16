//! The pipe server that runs inside the UIAccess OSD helper.
//!
//! Threading: [`OsdServer::serve`] runs on one thread and blocks it, with a helper thread of
//! its own for the `Hello` deadline. A [`Stopper`] may call [`Stopper::stop`] from any thread.
//!
//! Admission: a client counts as connected only after its first message, within
//! [`HELLO_TIMEOUT`], is a `Hello` naming the pid Windows reports for the pipe client
//! (`GetNamedPipeClientProcessId`). Anything else drops the client with
//! [`ServerEvent::Rejected`], so a silent or lying client cannot hold the single instance.

mod admission;
mod events;
mod hello_clock;

pub use admission::{HELLO_TIMEOUT, Rejection};
pub use events::{BindError, Flow, ServerEvent};

use std::mem::size_of;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{
    ERROR_ACCESS_DENIED, ERROR_BROKEN_PIPE, ERROR_MORE_DATA, ERROR_NO_DATA,
    ERROR_OPERATION_ABORTED, ERROR_PIPE_BUSY, ERROR_PIPE_CONNECTED, ERROR_PIPE_NOT_CONNECTED,
    FALSE, GetLastError, HLOCAL, LocalFree,
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
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, GetNamedPipeClientProcessId,
    PIPE_READMODE_MESSAGE, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_MESSAGE, PIPE_WAIT,
};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::core::PCWSTR;

use self::admission::admit;
use self::hello_clock::HelloClock;
use crate::identity::{self, ServerTrust};
use crate::security::token::FactsError;
use crate::security::{pipe_sddl, token};
use crate::win::{OwnedHandle, duplicate_current_thread, wide};
use crate::{DecodeError, MAX_MESSAGE, Message, decode};

#[derive(Default)]
struct Shared {
    stop: AtomicBool,
    serving_thread: AtomicU32,
    /// A real handle to the serving thread, present only while `serve` runs.
    thread: Mutex<Option<OwnedHandle>>,
    hello: HelloClock,
}

impl Shared {
    /// Aborts the serving thread's blocking pipe call, if `serve` runs.
    fn cancel_serving_io(&self) {
        let guard = self.thread.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(handle) = guard.as_ref() {
            // SAFETY: the handle stays valid while the guard is held.
            unsafe {
                let _ = CancelSynchronousIo(handle.0);
            }
        }
    }
}

/// How the first message of a connection ended.
enum Admission {
    Admitted { pid: u32, hello: Message },
    Rejected(Rejection),
    Stop,
    Failed(String),
}

/// Stops a running [`OsdServer::serve`] from any thread.
#[derive(Clone)]
pub struct Stopper {
    shared: Arc<Shared>,
}

impl Stopper {
    /// Asks the server to stop and aborts its blocking pipe call.
    ///
    /// During shutdown this may also cancel synchronous I/O that the message callback
    /// itself has in flight on the serving thread.
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
    Rejected,
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
    /// [`BindError::AlreadyRunning`] when this user's server already owns the name,
    /// [`BindError::Squatted`] when another user's (or a less trusted) process holds it,
    /// [`BindError::Os`] otherwise.
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
            return Err(if error == ERROR_ACCESS_DENIED {
                owner_of_taken_name(&wide_name)
            } else if error == ERROR_PIPE_BUSY {
                BindError::AlreadyRunning
            } else {
                BindError::Os(format!(
                    "CreateNamedPipeW: {}",
                    windows::core::Error::from(error)
                ))
            });
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

    /// Serves clients one after another and reports to `on`: [`ServerEvent::Connected`]
    /// once a client is admitted, then every message (the admitting `Hello` first), then
    /// [`ServerEvent::Disconnected`] when the client goes. A client that is not admitted is
    /// reported as [`ServerEvent::Rejected`] and dropped.
    ///
    /// Blocks the calling thread until `on` returns [`Flow::Stop`], the [`Stopper`] fires
    /// or a fatal error occurs. A client that sends a fatal decode error (bad magic or
    /// version) is disconnected and the next client is awaited.
    ///
    /// # Errors
    /// Returns the Windows error text on an unexpected pipe failure, or when the deadline
    /// thread cannot start.
    pub fn serve(&self, mut on: impl FnMut(ServerEvent) -> Flow) -> Result<(), String> {
        let _registration = Registration::new(&self.shared)?;
        let shared = &*self.shared;
        std::thread::scope(|scope| {
            std::thread::Builder::new()
                .name(String::from("plith-ipc-hello-clock"))
                .spawn_scoped(scope, || shared.hello.run(|| shared.cancel_serving_io()))
                .map_err(|e| format!("spawn the hello clock: {e}"))?;
            let result = self.serve_clients(&mut on);
            shared.hello.quit();
            result
        })
    }

    fn serve_clients(&self, on: &mut impl FnMut(ServerEvent) -> Flow) -> Result<(), String> {
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
            let outcome = match self.admit_client() {
                Admission::Admitted { pid, hello } => {
                    if on(ServerEvent::Connected { pid }) == Flow::Stop
                        || on(ServerEvent::Message(Ok(hello))) == Flow::Stop
                    {
                        Pumped::Stop
                    } else {
                        self.pump(on)
                    }
                }
                Admission::Rejected(why) => {
                    if on(ServerEvent::Rejected(why)) == Flow::Stop {
                        Pumped::Stop
                    } else {
                        Pumped::Rejected
                    }
                }
                Admission::Stop => Pumped::Stop,
                Admission::Failed(error) => Pumped::Failed(error),
            };
            // SAFETY: the pipe handle is valid; a failure only means nothing was connected.
            unsafe {
                let _ = DisconnectNamedPipe(self.pipe.0);
            }
            match outcome {
                Pumped::Stop => return Ok(()),
                Pumped::Rejected => {}
                Pumped::Disconnect => {
                    if on(ServerEvent::Disconnected) == Flow::Stop {
                        return Ok(());
                    }
                }
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

    /// Reads the connected client's pid and first message and decides admission, with the
    /// [`HELLO_TIMEOUT`] deadline armed for the read.
    fn admit_client(&self) -> Admission {
        let mut pid = 0u32;
        // SAFETY: the pipe handle is valid and connected; `pid` is a valid out pointer.
        if let Err(e) = unsafe { GetNamedPipeClientProcessId(self.pipe.0, &raw mut pid) } {
            return Admission::Rejected(Rejection::NoClientPid(e.to_string()));
        }
        let mut buffer = [0u8; MAX_MESSAGE];
        let clock = &self.shared.hello;
        clock.arm(Instant::now() + HELLO_TIMEOUT);
        let first = loop {
            if self.shared.stop.load(Ordering::SeqCst) {
                clock.disarm();
                return Admission::Stop;
            }
            match self.read_message(&mut buffer) {
                Read::Message(len) => break decode(&buffer[..len]),
                Read::TooBig => {
                    clock.disarm();
                    self.drain();
                    return Admission::Rejected(Rejection::NotHello);
                }
                Read::Closed => {
                    clock.disarm();
                    return Admission::Rejected(Rejection::ClosedBeforeHello);
                }
                Read::Aborted if clock.expired() => {
                    clock.disarm();
                    return Admission::Rejected(Rejection::Timeout);
                }
                Read::Aborted => {}
                Read::Failed(error) => {
                    clock.disarm();
                    return Admission::Failed(error);
                }
            }
        };
        clock.disarm();
        match admit(&first, pid) {
            Ok(hello) => Admission::Admitted { pid, hello },
            Err(why) => Admission::Rejected(why),
        }
    }

    fn pump(&self, on: &mut impl FnMut(ServerEvent) -> Flow) -> Pumped {
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
            if on(ServerEvent::Message(result)) == Flow::Stop {
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

/// Tells our own earlier server from a squatter when the name is already taken.
fn owner_of_taken_name(wide_name: &[u16]) -> BindError {
    match identity::open_client(wide_name) {
        // Our own earlier helper runs at our level, so the same-user rule tells it apart.
        Ok(probe) => match identity::server_is_trusted_for(&probe, ServerTrust::DevSameUser) {
            Ok(true) => BindError::AlreadyRunning,
            Ok(false) | Err(FactsError::AccessDenied) => BindError::Squatted,
            Err(FactsError::Os(error)) => BindError::Os(error),
        },
        Err(e) if e.code() == ERROR_PIPE_BUSY.to_hresult() => BindError::AlreadyRunning,
        Err(e) if e.code() == ERROR_ACCESS_DENIED.to_hresult() => BindError::Squatted,
        Err(e) => BindError::Os(format!("probing the pipe: {e}")),
    }
}

/// Publishes the serving thread to the [`Stopper`] while `serve` runs.
struct Registration<'a> {
    shared: &'a Shared,
}

impl<'a> Registration<'a> {
    fn new(shared: &'a Shared) -> Result<Self, String> {
        let duplicate = duplicate_current_thread()?;
        // SAFETY: plain query of the calling thread id.
        shared
            .serving_thread
            .store(unsafe { GetCurrentThreadId() }, Ordering::SeqCst);
        *shared.thread.lock().unwrap_or_else(PoisonError::into_inner) = Some(duplicate);
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
