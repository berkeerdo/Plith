//! The always-on close backstop: stopping the render thread must not hang the UI thread inside
//! `WM_CLOSE` with the island on screen. If the join takes too long the process exits.

use super::state::Stopper;
use super::wire::EXIT_RENDER_FATAL;
use crate::log::LogFile;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Duration;

/// How long the render join may take before the process is ended.
pub const CLOSE_BACKSTOP_MS: u64 = 3000;

/// Runs `work` with a timer beside it. If `work` has not returned after `timeout`, `on_timeout`
/// runs (once) while `work` keeps going; when `work` returns in time the timer is released at
/// once (also when `work` panics) and `on_timeout` never runs.
pub fn with_backstop<R>(
    timeout: Duration,
    on_timeout: impl FnOnce() + Send,
    work: impl FnOnce() -> R,
) -> R {
    let (release, released) = mpsc::channel::<()>();
    thread::scope(|scope| {
        scope.spawn(move || {
            // A send or a disconnect (the work returned or unwound) both mean "released".
            if released.recv_timeout(timeout) == Err(RecvTimeoutError::Timeout) {
                on_timeout();
            }
        });
        // Owned by a guard so an unwind of `work` drops it and disconnects the channel at once.
        let _release = release;
        work()
    })
}

/// Stops and joins the render thread under the backstop: after [`CLOSE_BACKSTOP_MS`] it logs,
/// flushes the log and ends the process with the fatal exit code.
pub fn close_render(log: &LogFile, stop: &dyn Stopper) {
    with_backstop(
        Duration::from_millis(CLOSE_BACKSTOP_MS),
        || {
            log.line(&format!(
                "close backstop: render join exceeded {CLOSE_BACKSTOP_MS} ms, forced exit"
            ));
            // `exit` skips every `Drop`: show any flyout the OSD suppression hid.
            crate::osd::restore_now();
            log.flush();
            std::process::exit(EXIT_RENDER_FATAL);
        },
        || stop.stop_and_join(),
    );
}

#[cfg(test)]
mod tests;
