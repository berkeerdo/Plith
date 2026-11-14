//! Thread-safe handles to the UI thread: the shape poster, the generic UI waker and the quit
//! handle. Every private `WM_APP + n` message id lives here.

use crate::handles::hwnd_of;
use plith_core::scene::{ShapeCell, ShapeFrame};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP, WM_CLOSE};

/// Posted to the UI thread when a new shape is waiting in the cell.
pub(crate) const WM_APP_SHAPE: u32 = WM_APP + 1;

/// Callback message of the `AppBar` registration (which never positions an edge).
pub(crate) const WM_APP_BAR: u32 = WM_APP + 2;

/// Posted to the UI thread by [`UiWaker`]; surfaces as `ShellEvent::Wake`.
pub(crate) const WM_APP_WAKE: u32 = WM_APP + 3;

/// Coalesces posts: while one message is queued, further posts are skipped. The UI thread calls
/// [`Coalesce::clear`] when it dequeues the message.
#[derive(Debug, Default)]
pub(crate) struct Coalesce(AtomicBool);

impl Coalesce {
    /// Marks a post as queued. Returns `true` when the caller must post, `false` when a post is
    /// already queued.
    pub(crate) fn arm(&self) -> bool {
        !self.0.swap(true, Ordering::AcqRel)
    }

    /// Allows the next post; called by the UI thread when the queued message is handled.
    pub(crate) fn clear(&self) {
        self.0.store(false, Ordering::Release);
    }

    /// Runs `post` unless a post is already queued. A failed post (`false`) clears the flag so a
    /// later call can retry. Returns whether `post` ran and succeeded.
    pub(crate) fn post_with(&self, post: impl FnOnce() -> bool) -> bool {
        if !self.arm() {
            return false;
        }
        if post() {
            true
        } else {
            self.clear();
            false
        }
    }
}

/// Posts `msg` to `hwnd` with empty parameters; `true` when it was queued.
fn post_message(hwnd: isize, msg: u32) -> bool {
    // SAFETY: PostMessageW is thread safe and accepts any handle value; a stale handle only
    // makes it fail.
    unsafe { PostMessageW(Some(hwnd_of(hwnd)), msg, WPARAM(0), LPARAM(0)) }.is_ok()
}

/// Lets the render thread hand a drawn shape to the UI thread. Posts are coalesced: while one
/// message is queued, later frames only replace the cell content.
#[derive(Debug, Clone)]
pub struct ShapePoster {
    pub(crate) hwnd: isize,
    pub(crate) cell: Arc<ShapeCell>,
    pub(crate) pending: Arc<Coalesce>,
}

impl ShapePoster {
    /// Stores the frame and wakes the UI thread unless a wake-up is already queued.
    /// A failed post (window gone) clears the pending flag and is otherwise ignored.
    pub fn post(&self, frame: ShapeFrame) {
        self.cell.publish(frame);
        self.pending
            .post_with(|| post_message(self.hwnd, WM_APP_SHAPE));
    }
}

/// Wakes the UI thread from any thread; it surfaces as `ShellEvent::Wake`. Service threads call
/// it from their `set_waker` closure. Wakes are coalesced: while one is queued, later calls do
/// nothing, so the handler must re-read whatever state the services published.
///
/// Invariant: render and service threads only post to the UI thread, never send (no
/// `SendMessage`, no blocking on a UI reply). The UI thread blocks on and joins those threads,
/// so a send from them could deadlock.
#[derive(Debug, Clone)]
pub struct UiWaker {
    pub(crate) hwnd: isize,
    pub(crate) pending: Arc<Coalesce>,
}

impl UiWaker {
    /// Posts one `WM_APP_WAKE` unless one is already queued. A failed post (window gone) clears
    /// the flag and is otherwise ignored.
    pub fn wake(&self) {
        self.pending
            .post_with(|| post_message(self.hwnd, WM_APP_WAKE));
    }
}

/// Asks the UI thread to close the window and leave the message loop.
#[derive(Debug, Clone, Copy)]
pub struct QuitHandle {
    pub(crate) hwnd: isize,
}

impl QuitHandle {
    /// Posts `WM_CLOSE`; the window's `WM_DESTROY` then ends the message loop.
    pub fn quit(&self) {
        let _ = post_message(self.hwnd, WM_CLOSE);
    }
}

#[cfg(test)]
mod tests;
