//! The non-blocking pipe client used by the notch.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::ERROR_PIPE_BUSY;
use windows::Win32::Storage::FileSystem::WriteFile;
use windows::Win32::System::IO::CancelSynchronousIo;
use windows::Win32::System::Pipes::WaitNamedPipeW;
use windows::core::PCWSTR;

use crate::identity::{open_client, server_is_trusted_for};
use crate::latest::Slot;
use crate::security::token::FactsError;
use crate::win::{OwnedHandle, duplicate_current_thread, wide};
use crate::{EncodeError, Message, encode};

const RETRY: Duration = Duration::from_secs(1);
const POLL: Duration = Duration::from_millis(500);
/// How long drop waits for the worker before it cancels the worker's blocking call.
const DROP_GRACE: Duration = Duration::from_millis(500);
/// How long drop waits after cancelling before it gives up on the worker.
const DROP_CANCEL: Duration = Duration::from_millis(500);
/// The `last_error` text when the pipe is served by a process this client must not trust.
const UNTRUSTED: &str =
    "the pipe server is not trusted (another user, an AppContainer or a lower integrity level)";

/// A link to the OSD helper that never blocks the caller.
///
/// A worker thread connects, reconnects at most once per second and forwards the latest
/// pending message. Dropping the link replaces any message not yet sent with `Bye`, which
/// implies hiding the overlay, and returns within about one second even if the server has
/// stopped reading.
///
/// [`is_connected`](Self::is_connected) and [`last_error`](Self::last_error) report what
/// the worker last saw, without blocking.
pub struct OsdLink {
    slot: Arc<Slot<Message>>,
    worker: Option<JoinHandle<()>>,
    state: Arc<WorkerState>,
    status: Arc<LinkStatus>,
}

/// What the worker last saw of the pipe, readable from any thread without waiting on it.
#[derive(Default)]
struct LinkStatus {
    connected: AtomicBool,
    error: Mutex<Option<String>>,
}

impl LinkStatus {
    /// A connect (open, trust check, `Hello`) succeeded: up, and the error is cleared.
    fn connected(&self) {
        *WorkerState::lock(&self.error) = None;
        self.connected.store(true, Ordering::SeqCst);
    }

    /// A connect or a write failed: down, and `text` is the latest error.
    fn failed(&self, text: String) {
        self.connected.store(false, Ordering::SeqCst);
        *WorkerState::lock(&self.error) = Some(text);
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    fn last_error(&self) -> Option<String> {
        WorkerState::lock(&self.error).clone()
    }
}

/// What drop needs to know about the worker.
#[derive(Default)]
struct WorkerState {
    /// A real handle to the worker thread while it runs, used to cancel a blocked write.
    thread: Mutex<Option<OwnedHandle>>,
    done: Mutex<bool>,
    finished: Condvar,
}

impl WorkerState {
    fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
        mutex.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn wait_done(&self, limit: Duration) -> bool {
        let deadline = Instant::now() + limit;
        let mut done = Self::lock(&self.done);
        while !*done {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return false;
            }
            done = self
                .finished
                .wait_timeout(done, left)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
        true
    }

    fn cancel_worker_io(&self) {
        if let Some(handle) = Self::lock(&self.thread).as_ref() {
            // SAFETY: the handle stays valid while the guard is held.
            unsafe {
                let _ = CancelSynchronousIo(handle.0);
            }
        }
    }
}

/// Publishes the worker thread to drop and signals when the worker ends.
struct Registration<'a>(&'a WorkerState);

impl<'a> Registration<'a> {
    fn new(state: &'a WorkerState) -> Self {
        if let Ok(handle) = duplicate_current_thread() {
            *WorkerState::lock(&state.thread) = Some(handle);
        }
        Self(state)
    }
}

impl Drop for Registration<'_> {
    fn drop(&mut self) {
        *WorkerState::lock(&self.0.thread) = None;
        *WorkerState::lock(&self.0.done) = true;
        self.0.finished.notify_all();
    }
}

impl OsdLink {
    /// Starts the worker for the pipe `name`. The pipe need not exist yet.
    #[must_use]
    pub fn start(name: String) -> Self {
        let slot = Arc::new(Slot::new());
        let state = Arc::new(WorkerState::default());
        let status = Arc::new(LinkStatus::default());
        let worker = {
            let slot = Arc::clone(&slot);
            let state = Arc::clone(&state);
            let status = Arc::clone(&status);
            std::thread::spawn(move || {
                let _registration = Registration::new(&state);
                run(&name, &slot, &status);
            })
        };
        Self {
            slot,
            worker: Some(worker),
            state,
            status,
        }
    }

    /// Whether the worker holds a connection to a trusted server that took `Hello`.
    ///
    /// True from a successful connect until a write or the next connect fails. A server
    /// that goes away while nothing is sent is noticed at the next send, so pair this with
    /// [`HelperProcess::has_exited`](crate::HelperProcess::has_exited). Never blocks.
    #[must_use]
    pub fn is_connected(&self) -> bool {
        self.status.is_connected()
    }

    /// The latest connect or write failure (no server, busy, untrusted server, broken pipe),
    /// or `None` since the last successful connect. Never waits on the worker: the lock it
    /// takes is only held to swap the text.
    #[must_use]
    pub fn last_error(&self) -> Option<String> {
        self.status.last_error()
    }

    /// Queues a message; a message not yet sent is replaced. Never blocks.
    ///
    /// # Errors
    /// Returns [`EncodeError`] when the message is out of range.
    pub fn send(&self, message: Message) -> Result<(), EncodeError> {
        encode(&message)?;
        self.slot.put(message);
        Ok(())
    }
}

impl Drop for OsdLink {
    fn drop(&mut self) {
        self.slot.put(Message::Bye);
        self.slot.close();
        let mut finished = self.state.wait_done(DROP_GRACE);
        if !finished {
            // The worker is probably blocked in a write to a server that stopped reading.
            // It may not be inside the call yet, so cancel repeatedly.
            for _ in 0..10 {
                self.state.cancel_worker_io();
                finished = self.state.wait_done(DROP_CANCEL / 10);
                if finished {
                    break;
                }
            }
        }
        if let Some(worker) = self.worker.take()
            && finished
        {
            let _ = worker.join();
        }
        // An unfinished worker is detached; it exits as soon as its call returns.
    }
}

fn run(name: &str, slot: &Slot<Message>, status: &LinkStatus) {
    let wide_name = wide(name);
    let mut connection: Option<OwnedHandle> = None;
    let mut carry: Option<Message> = None;
    let mut next_attempt = Instant::now();
    let mut connected_at = next_attempt;
    loop {
        if let Some(newer) = slot.take_wait(Duration::ZERO) {
            carry = Some(newer);
        }
        let Some(pipe) = connection.as_ref() else {
            if slot.is_closed() {
                return;
            }
            let now = Instant::now();
            if now < next_attempt {
                if let Some(newer) = slot.take_wait(next_attempt - now) {
                    carry = Some(newer);
                }
                continue;
            }
            connected_at = Instant::now();
            match connect(&wide_name) {
                Ok(pipe) => {
                    status.connected();
                    connection = Some(pipe);
                }
                Err(error) => {
                    status.failed(error);
                    next_attempt = connected_at + RETRY;
                }
            }
            continue;
        };
        if carry.is_none() {
            carry = slot.take_wait(POLL);
            if carry.is_none() {
                if slot.is_closed() {
                    return;
                }
                continue;
            }
        }
        let Some(message) = carry.take() else {
            continue;
        };
        let sent = write(pipe, &message);
        if message == Message::Bye {
            return;
        }
        if let Err(error) = sent {
            status.failed(format!("write: {error}"));
            connection = None;
            // A connection that died within a second must not cause a tight reconnect loop.
            next_attempt = connected_at + RETRY;
            carry = Some(message);
        }
    }
}

/// Opens the pipe, checks that this user's process serves it and sends `Hello`.
/// The error text says why not: no server, busy, not ours, or the `Hello` write failed.
fn connect(wide_name: &[u16]) -> Result<OwnedHandle, String> {
    let pipe = match open_client(wide_name) {
        Ok(pipe) => pipe,
        Err(e) if e.code() == ERROR_PIPE_BUSY.to_hresult() => {
            // SAFETY: `wide_name` is NUL-terminated and outlives the call.
            if !unsafe { WaitNamedPipeW(PCWSTR(wide_name.as_ptr()), 500) }.as_bool() {
                return Err(String::from("open: the pipe stayed busy"));
            }
            open_client(wide_name).map_err(|e| format!("open: {e}"))?
        }
        Err(e) => return Err(format!("open: {e}")),
    };
    // Never send to a server of another user, in an AppContainer or at a lower integrity
    // level: close and try again later.
    match server_is_trusted_for(&pipe) {
        Ok(true) => {}
        Ok(false) => return Err(String::from(UNTRUSTED)),
        Err(FactsError::AccessDenied) => {
            return Err(format!("{UNTRUSTED}: its process cannot be opened"));
        }
        Err(FactsError::Os(error)) => return Err(format!("checking the pipe server: {error}")),
    }
    let hello = Message::Hello {
        client_pid: std::process::id(),
    };
    write(&pipe, &hello).map_err(|e| format!("hello: {e}"))?;
    Ok(pipe)
}

fn write(pipe: &OwnedHandle, message: &Message) -> Result<(), String> {
    let bytes = encode(message).map_err(|e| e.to_string())?;
    let mut written = 0u32;
    // SAFETY: the handle is open and the buffer outlives the synchronous call.
    unsafe { WriteFile(pipe.0, Some(&bytes), Some(&raw mut written), None) }
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests;
