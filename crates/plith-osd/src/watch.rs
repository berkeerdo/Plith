//! Watches the connected client process and reports its end to the render thread.
//!
//! The pid comes from `ServerEvent::Connected`, which `plith-ipc` reads from the kernel
//! (`GetNamedPipeClientProcessId`), never from message bytes. One [`Watcher`] runs per
//! connection; dropping it cancels and joins its thread, so no watcher outlives its client.

use crate::log::Log;
use crate::render::Event;
use std::sync::Arc;
use std::sync::mpsc::SyncSender;
use std::thread::{self, JoinHandle};
use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
use windows::Win32::System::Threading::{
    CreateEventW, INFINITE, OpenProcess, PROCESS_SYNCHRONIZE, SetEvent, WaitForMultipleObjects,
};

/// An owned kernel handle, closed on drop.
struct Owned(HANDLE);

// SAFETY: a kernel handle is a process-wide token that any thread may wait on or close.
unsafe impl Send for Owned {}
// SAFETY: see `Send`; the wrapper adds no unsynchronised state.
unsafe impl Sync for Owned {}

impl Drop for Owned {
    fn drop(&mut self) {
        // SAFETY: the handle is owned by this value and closed exactly once.
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

/// A thread blocked on one client process. Sends [`Event::ClientGone`] when the process
/// ends (at once when it cannot be opened, for example because it has already ended; the
/// open error is logged). Dropping it ends the thread without sending anything.
pub struct Watcher {
    cancel: Arc<Owned>,
    thread: Option<JoinHandle<()>>,
}

impl Drop for Watcher {
    fn drop(&mut self) {
        // SAFETY: the event handle is alive while `self.cancel` holds it.
        unsafe {
            let _ = SetEvent(self.cancel.0);
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Starts a watcher for the client process `pid`.
///
/// # Errors
/// Returns why the cancel event or the thread could not be created.
pub fn client(pid: u32, events: SyncSender<Event>, log: Arc<Log>) -> Result<Watcher, String> {
    // SAFETY: an unnamed manual-reset event, initially clear; owned by `Owned`.
    let cancel = unsafe { CreateEventW(None, true, false, None) }
        .map(|handle| Arc::new(Owned(handle)))
        .map_err(|e| format!("cancel event: {e}"))?;
    let thread_cancel = Arc::clone(&cancel);
    let thread = thread::Builder::new()
        .name(format!("plith-osd-watch-{pid}"))
        .spawn(move || {
            if wait_for_end(pid, &thread_cancel, &log) {
                let _ = events.send(Event::ClientGone(pid));
            }
        })
        .map_err(|e| format!("spawn watcher: {e}"))?;
    Ok(Watcher {
        cancel,
        thread: Some(thread),
    })
}

/// Blocks until the process ends (true) or `cancel` is set (false). A process that cannot
/// be opened counts as ended.
fn wait_for_end(pid: u32, cancel: &Owned, log: &Log) -> bool {
    // SAFETY: opening a process by id with the wait right only; owned by `Owned`.
    let process = match unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, pid) } {
        Ok(process) => Owned(process),
        Err(error) => {
            log.line(&format!(
                "osd: cannot watch client {pid} ({error}), treating it as gone"
            ));
            return true;
        }
    };
    // SAFETY: both handles are alive for the duration of the wait.
    let woke = unsafe { WaitForMultipleObjects(&[process.0, cancel.0], false, INFINITE) };
    woke == WAIT_OBJECT_0
}

#[cfg(test)]
mod tests;
