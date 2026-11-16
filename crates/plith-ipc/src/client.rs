//! The non-blocking pipe client used by the notch.

use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::ERROR_PIPE_BUSY;
use windows::Win32::Storage::FileSystem::WriteFile;
use windows::Win32::System::IO::CancelSynchronousIo;
use windows::Win32::System::Pipes::WaitNamedPipeW;
use windows::core::PCWSTR;

use crate::identity::{open_client, server_is_current_user};
use crate::latest::Slot;
use crate::win::{OwnedHandle, duplicate_current_thread, wide};
use crate::{EncodeError, Message, encode};

const RETRY: Duration = Duration::from_secs(1);
const POLL: Duration = Duration::from_millis(500);
/// How long drop waits for the worker before it cancels the worker's blocking call.
const DROP_GRACE: Duration = Duration::from_millis(500);
/// How long drop waits after cancelling before it gives up on the worker.
const DROP_CANCEL: Duration = Duration::from_millis(500);

/// A link to the OSD helper that never blocks the caller.
///
/// A worker thread connects, reconnects at most once per second and forwards the latest
/// pending message. Dropping the link replaces any message not yet sent with `Bye`, which
/// implies hiding the overlay, and returns within about one second even if the server has
/// stopped reading.
pub struct OsdLink {
    slot: Arc<Slot<Message>>,
    worker: Option<JoinHandle<()>>,
    state: Arc<WorkerState>,
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
        let worker = {
            let slot = Arc::clone(&slot);
            let state = Arc::clone(&state);
            std::thread::spawn(move || {
                let _registration = Registration::new(&state);
                run(&name, &slot);
            })
        };
        Self {
            slot,
            worker: Some(worker),
            state,
        }
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

fn run(name: &str, slot: &Slot<Message>) {
    let wide_name = wide(name);
    let mut connection: Option<OwnedHandle> = None;
    let mut carry: Option<Message> = None;
    let mut next_attempt = Instant::now();
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
            connection = connect(&wide_name);
            if connection.is_none() {
                next_attempt = Instant::now() + RETRY;
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
        let sent = write(pipe, &message).is_ok();
        if message == Message::Bye {
            return;
        }
        if !sent {
            connection = None;
            carry = Some(message);
        }
    }
}

/// Opens the pipe, checks that this user's process serves it and sends `Hello`.
/// `None` when the server is not there or is not ours.
fn connect(wide_name: &[u16]) -> Option<OwnedHandle> {
    let pipe = match open_client(wide_name) {
        Ok(pipe) => pipe,
        Err(e) if e.code() == ERROR_PIPE_BUSY.to_hresult() => {
            // SAFETY: `wide_name` is NUL-terminated and outlives the call.
            if !unsafe { WaitNamedPipeW(PCWSTR(wide_name.as_ptr()), 500) }.as_bool() {
                return None;
            }
            open_client(wide_name).ok()?
        }
        Err(_) => return None,
    };
    // Never send to a server that runs as another user: close and try again later.
    if !server_is_current_user(&pipe).ok()? {
        return None;
    }
    let hello = Message::Hello {
        client_pid: std::process::id(),
    };
    write(&pipe, &hello).ok()?;
    Some(pipe)
}

fn write(pipe: &OwnedHandle, message: &Message) -> Result<(), String> {
    let bytes = encode(message).map_err(|e| e.to_string())?;
    let mut written = 0u32;
    // SAFETY: the handle is open and the buffer outlives the synchronous call.
    unsafe { WriteFile(pipe.0, Some(&bytes), Some(&raw mut written), None) }
        .map_err(|e| e.to_string())
}
