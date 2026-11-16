//! Watches the client process named in `Hello` and reports its end to the render thread.

use crate::render::Event;
use std::sync::mpsc::SyncSender;
use std::thread;
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Threading::{
    INFINITE, OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject,
};

/// Starts a detached thread that blocks on the process `pid` and sends
/// [`Event::ClientGone`] once it ends (at once when it cannot be opened, for example because
/// it has already ended). The thread holds no lock and ends with the process it watches, or
/// with this process.
///
/// # Errors
/// Returns the spawn failure.
pub fn client(pid: u32, events: SyncSender<Event>) -> Result<(), String> {
    thread::Builder::new()
        .name(format!("plith-osd-watch-{pid}"))
        .spawn(move || {
            // SAFETY: opening a process by id with the wait right only; the handle is closed
            // below.
            if let Ok(process) = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, pid) } {
                // SAFETY: `process` is a live handle with SYNCHRONIZE access; it is closed once.
                unsafe {
                    let _ = WaitForSingleObject(process, INFINITE);
                    let _ = CloseHandle(process);
                }
            }
            let _ = events.send(Event::ClientGone(pid));
        })
        .map(|_| ())
        .map_err(|e| format!("spawn watcher: {e}"))
}
