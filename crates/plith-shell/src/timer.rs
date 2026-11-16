//! App timers on the notch window: the app sets and kills them on the UI thread and each tick
//! surfaces as `ShellEvent::Timer(id)`. Ids below [`FIRST_APP_TIMER`] belong to the shell.

use crate::handles::hwnd_of;
use std::marker::PhantomData;
use windows::Win32::UI::WindowsAndMessaging::{KillTimer, SetTimer};

/// The lowest timer id the app may use; lower ids are the shell's own.
pub const FIRST_APP_TIMER: usize = 0x100;

/// True when a `WM_TIMER` id is an app timer.
#[must_use]
pub fn is_app_timer(id: usize) -> bool {
    id >= FIRST_APP_TIMER
}

/// Sets and kills app timers. Not `Send`: `SetTimer` only works for a window of the calling
/// thread.
#[derive(Debug, Clone)]
pub struct TimerHandle {
    pub(crate) hwnd: isize,
    pub(crate) _ui_thread: PhantomData<*const ()>,
}

impl TimerHandle {
    /// Starts or restarts timer `id` (at least [`FIRST_APP_TIMER`]) with a period of `ms`.
    /// Ticks repeat until [`kill`](Self::kill). A failure (window gone) is ignored.
    pub fn set(&self, id: usize, ms: u32) {
        debug_assert!(is_app_timer(id), "timer id {id} is the shell's");
        // SAFETY: the handle is the notch window, owned by this thread (`TimerHandle` is not
        // `Send`); a stale handle only makes the call fail. No callback: ticks are WM_TIMER.
        let _ = unsafe { SetTimer(Some(hwnd_of(self.hwnd)), id, ms, None) };
    }

    /// Stops timer `id`; stopping one that does not run is fine.
    pub fn kill(&self, id: usize) {
        // SAFETY: as in `set`; KillTimer fails harmlessly when no such timer runs.
        let _ = unsafe { KillTimer(Some(hwnd_of(self.hwnd)), id) };
    }
}

#[cfg(test)]
mod tests;
