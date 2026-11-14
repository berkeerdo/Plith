//! A thread that installs hooks and pumps messages until told to quit.
//!
//! Both a `WinEvent` hook (out of context) and a low level keyboard hook are delivered through
//! the message loop of the thread that installed them, so each owner needs such a thread. The
//! guard returned by the setup closure (the installed hooks) is created on the thread, and
//! dropped there when the loop ends, on every exit path including a panic.

use std::sync::mpsc::channel;
use std::thread::{self, JoinHandle};

use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, MSG, PM_NOREMOVE, PeekMessageW, PostThreadMessageW,
    TranslateMessage, WM_QUIT,
};

/// Handle to a running hook thread. Dropping it stops and joins the thread.
#[derive(Debug)]
pub(crate) struct HookThread {
    thread_id: u32,
    join: Option<JoinHandle<()>>,
}

impl HookThread {
    /// Starts the thread, runs `setup` on it and waits for the result. The value `setup`
    /// returns lives until the message loop ends and is then dropped on that thread.
    pub(crate) fn spawn<G: 'static>(
        name: &str,
        setup: impl FnOnce() -> Result<G, String> + Send + 'static,
    ) -> Result<Self, String> {
        let (ready_tx, ready_rx) = channel::<Result<u32, String>>();
        let join = thread::Builder::new()
            .name(name.to_owned())
            .spawn(move || {
                // Force the thread's message queue into existence before anyone posts to it.
                let mut msg = MSG::default();
                // SAFETY: `msg` is a valid out pointer; PM_NOREMOVE only creates the queue.
                let _ = unsafe { PeekMessageW(&raw mut msg, None, 0, 0, PM_NOREMOVE) };
                match setup() {
                    Ok(guard) => {
                        // SAFETY: no arguments.
                        let id = unsafe { GetCurrentThreadId() };
                        if ready_tx.send(Ok(id)).is_ok() {
                            pump();
                        }
                        drop(guard);
                    }
                    Err(why) => {
                        let _ = ready_tx.send(Err(why));
                    }
                }
            })
            .map_err(|e| format!("cannot start {name} thread: {e}"))?;
        match ready_rx.recv() {
            Ok(Ok(thread_id)) => Ok(Self {
                thread_id,
                join: Some(join),
            }),
            Ok(Err(why)) => {
                let _ = join.join();
                Err(why)
            }
            Err(_) => {
                let _ = join.join();
                Err(format!("{name} thread ended before it was ready"))
            }
        }
    }

    /// Posts `WM_QUIT` and joins; the thread unhooks (and restores) on its way out.
    /// Safe to call twice.
    ///
    /// # Errors
    /// The post failed and the thread is still running (nothing would ever wake it), so it is
    /// not joined; the handle is kept and a later call tries again.
    pub(crate) fn stop(&mut self) -> Result<(), String> {
        let Some(join) = self.join.take() else {
            return Ok(());
        };
        // SAFETY: plain message post to a thread id this struct owns.
        let posted = unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) };
        match posted {
            Err(err) if !join.is_finished() => {
                self.join = Some(join);
                Err(format!("could not post WM_QUIT to the hook thread: {err}"))
            }
            _ => {
                let _ = join.join();
                Ok(())
            }
        }
    }
}

impl Drop for HookThread {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

/// Runs the message loop until `WM_QUIT` (or an error).
fn pump() {
    let mut msg = MSG::default();
    // SAFETY: `msg` is a valid out pointer for the whole loop; the thread owns its queue.
    unsafe {
        while GetMessageW(&raw mut msg, None, 0, 0).0 > 0 {
            let _ = TranslateMessage(&raw const msg);
            DispatchMessageW(&raw const msg);
        }
    }
}
