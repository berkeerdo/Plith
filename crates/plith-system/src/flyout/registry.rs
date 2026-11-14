//! Remembers the windows this process hid, so they can be put back.
//!
//! The shell creates its flyout once on some builds and only moves it, so a window left hidden
//! is a volume OSD that never returns, and exiting does not bring it back (hooks die with the
//! process, hidden windows do not). Free of Win32 types: hide and show arrive as closures.

use std::collections::BTreeSet;

/// Handles (as integers) hidden and not yet restored.
#[derive(Debug, Default)]
pub struct HiddenRegistry {
    hidden: BTreeSet<isize>,
}

impl HiddenRegistry {
    /// An empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// How many windows are owed a restore.
    #[must_use]
    pub fn len(&self) -> usize {
        self.hidden.len()
    }

    /// True when nothing is owed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.hidden.is_empty()
    }

    /// Hides `hwnd`: only a window that `is_visible` reports visible right now is hidden, and it
    /// is recorded only when `post_hide` reports the request was accepted. `post_hide` is an
    /// asynchronous request (`ShowWindowAsync`), so its result says nothing about the previous
    /// visibility; that is what `is_visible` is for. A window somebody else hid is not ours to
    /// show. Returns whether the window is now owed a restore.
    pub fn hide(
        &mut self,
        hwnd: isize,
        is_visible: impl FnOnce(isize) -> bool,
        post_hide: impl FnOnce(isize) -> bool,
    ) -> bool {
        if !is_visible(hwnd) || !post_hide(hwnd) {
            return false;
        }
        self.hidden.insert(hwnd);
        true
    }

    /// Calls `show` once for every recorded window, then forgets them all (a destroyed handle
    /// cannot be restored later, so holding it would only retry forever).
    pub fn restore_all(&mut self, mut show: impl FnMut(isize)) {
        for hwnd in std::mem::take(&mut self.hidden) {
            show(hwnd);
        }
    }
}
