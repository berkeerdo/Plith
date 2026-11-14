//! Thread-safe handles to the UI thread: the shape poster and the quit handle.

use plith_core::scene::{ShapeCell, ShapeFrame};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP, WM_CLOSE};

/// Posted to the UI thread when a new shape is waiting in the cell.
pub(crate) const WM_APP_SHAPE: u32 = WM_APP + 1;

fn hwnd_of(raw: isize) -> HWND {
    HWND(raw as *mut core::ffi::c_void)
}

/// Lets the render thread hand a drawn shape to the UI thread. Posts are coalesced: while one
/// message is queued, later frames only replace the cell content.
#[derive(Debug, Clone)]
pub struct ShapePoster {
    pub(crate) hwnd: isize,
    pub(crate) cell: Arc<ShapeCell>,
    pub(crate) pending: Arc<AtomicBool>,
}

impl ShapePoster {
    /// Stores the frame and wakes the UI thread unless a wake-up is already queued.
    /// A failed post (window gone) clears the pending flag and is otherwise ignored.
    pub fn post(&self, frame: ShapeFrame) {
        self.cell.publish(frame);
        if !self.pending.swap(true, Ordering::AcqRel) {
            // SAFETY: PostMessageW is thread safe and accepts any handle value; a stale handle
            // only makes it fail.
            let posted = unsafe {
                PostMessageW(Some(hwnd_of(self.hwnd)), WM_APP_SHAPE, WPARAM(0), LPARAM(0))
            };
            if posted.is_err() {
                self.pending.store(false, Ordering::Release);
            }
        }
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
        // SAFETY: PostMessageW is thread safe; a stale handle only makes it fail.
        let _ = unsafe { PostMessageW(Some(hwnd_of(self.hwnd)), WM_CLOSE, WPARAM(0), LPARAM(0)) };
    }
}
