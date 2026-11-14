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

    /// Hides `hwnd` through `hide`, which returns whether the window was visible until now
    /// (what `ShowWindow` returns). Only a real visible to hidden transition is recorded: a
    /// window somebody else hid is not ours to show.
    pub fn hide(&mut self, hwnd: isize, hide: impl FnOnce(isize) -> bool) {
        if hide(hwnd) {
            self.hidden.insert(hwnd);
        }
    }

    /// Calls `show` once for every recorded window, then forgets them all (a destroyed handle
    /// cannot be restored later, so holding it would only retry forever).
    pub fn restore_all(&mut self, mut show: impl FnMut(isize)) {
        for hwnd in std::mem::take(&mut self.hidden) {
            show(hwnd);
        }
    }
}
