//! Remembers the windows this process hid, so they can be put back.
//!
//! The shell creates its flyout once on some builds and only moves it, so a window left hidden
//! is a volume OSD that never returns, and exiting does not bring it back (hooks die with the
//! process, hidden windows do not). Free of Win32 types: hide and show arrive as closures.
//!
//! [`RestoreHandle`] shares one registry between the hook thread (which hides) and any thread
//! that must give the windows back right now (a backstop or watchdog before `process::exit`,
//! which skips every `Drop`). Each change is mirrored to the journal file under the same lock.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use super::journal;

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

    /// The recorded handles in ascending order.
    #[must_use]
    pub fn handles(&self) -> Vec<isize> {
        self.hidden.iter().copied().collect()
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

/// A cloneable, thread-safe handle to the hidden set of one suppressor
/// (from [`FlyoutSuppressor::restore_handle`](crate::FlyoutSuppressor::restore_handle)).
///
/// [`restore_now`](Self::restore_now) may run on any thread at any time, also right before
/// `std::process::exit`: it takes a short lock (poison tolerant), posts one asynchronous show
/// per window and clears the journal. It never waits on the shell.
#[derive(Debug, Clone)]
pub struct RestoreHandle {
    hidden: Arc<Mutex<HiddenRegistry>>,
    journal: Option<PathBuf>,
    show: fn(isize),
}

impl RestoreHandle {
    /// An empty set mirrored to `journal` (none: no file is written). `show` puts one window
    /// back and must not block (in the app: `ShowWindowAsync(SW_SHOWNA)`).
    pub(crate) fn new(journal: Option<PathBuf>, show: fn(isize)) -> Self {
        Self {
            hidden: Arc::new(Mutex::new(HiddenRegistry::new())),
            journal,
            show,
        }
    }

    /// The set; a panic while it was held cannot leave it half changed, so poison is ignored.
    fn lock(&self) -> MutexGuard<'_, HiddenRegistry> {
        self.hidden.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// [`HiddenRegistry::hide`] on the shared set; a window that is now owed a restore is
    /// written to the journal before the lock is released (best effort: a failed write is
    /// ignored, the in-memory set still restores it).
    pub(crate) fn hide(
        &self,
        hwnd: isize,
        is_visible: impl FnOnce(isize) -> bool,
        post_hide: impl FnOnce(isize) -> bool,
    ) -> bool {
        let mut set = self.lock();
        let hidden = set.hide(hwnd, is_visible, post_hide);
        if hidden && let Some(path) = &self.journal {
            let _ = journal::write(path, &set.handles());
        }
        hidden
    }

    /// How many windows are owed a restore.
    #[must_use]
    pub fn pending(&self) -> usize {
        self.lock().len()
    }

    /// Shows every hidden window now, empties the set and clears the journal. Callable from
    /// any thread, and idempotent: a second call finds nothing to do.
    pub fn restore_now(&self) {
        let mut set = self.lock();
        set.restore_all(self.show);
        if let Some(path) = &self.journal {
            let _ = journal::clear(path);
        }
    }

    /// Poisons the lock by unwinding while holding it (without the panic hook, so silently).
    #[cfg(test)]
    pub(crate) fn poison_for_test(&self) {
        let _held = self.lock();
        std::panic::resume_unwind(Box::new("poison"));
    }
}
