//! Process-wide setup.

use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::Shell::{
    QUNS_BUSY, QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN, SHQueryUserNotificationState,
};

/// Makes the process per-monitor DPI aware (v2). Call once, before creating any window.
///
/// # Errors
/// Returns the Win32 error, for example when the awareness was already set.
pub fn init_process() -> windows::core::Result<()> {
    // SAFETY: the call takes a plain constant and has no pointer arguments.
    unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) }
}

/// Names the reason the user must not be disturbed right now (an exclusive fullscreen game, a
/// busy state or presentation mode), or `None` when it is fine to show a window.
#[must_use]
pub fn user_busy() -> Option<&'static str> {
    // SAFETY: the call has no arguments and returns the state by value.
    let state = unsafe { SHQueryUserNotificationState() }.ok()?;
    match state {
        s if s == QUNS_RUNNING_D3D_FULL_SCREEN => Some("QUNS_RUNNING_D3D_FULL_SCREEN"),
        s if s == QUNS_BUSY => Some("QUNS_BUSY"),
        s if s == QUNS_PRESENTATION_MODE => Some("QUNS_PRESENTATION_MODE"),
        _ => None,
    }
}
