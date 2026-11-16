//! Process-wide setup.

use windows::Win32::System::Ole::{OleInitialize, OleUninitialize};
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

/// Makes the calling thread a single-threaded apartment with OLE (needed for drag and drop).
/// Call on the UI thread before creating the window; pair a success with [`uninit_ui_thread`].
///
/// # Errors
/// Returns the COM error when the apartment cannot be entered.
pub fn init_ui_thread() -> windows::core::Result<()> {
    // SAFETY: the reserved argument must be null, which `None` passes.
    unsafe { OleInitialize(None) }
}

/// Leaves the apartment entered by a successful [`init_ui_thread`], on the same thread.
pub fn uninit_ui_thread() {
    // SAFETY: balances one successful `OleInitialize` on this thread.
    unsafe { OleUninitialize() }
}

/// Names the reason the user must not be disturbed right now (an exclusive fullscreen game, a
/// busy state or presentation mode), or `None` when it is fine to show a window. Fails closed:
/// when the query itself fails it reports `QUNS_QUERY_FAILED`.
#[must_use]
pub fn user_busy() -> Option<&'static str> {
    // SAFETY: the call has no arguments and returns the state by value.
    let Ok(state) = (unsafe { SHQueryUserNotificationState() }) else {
        return Some("QUNS_QUERY_FAILED");
    };
    match state {
        s if s == QUNS_RUNNING_D3D_FULL_SCREEN => Some("QUNS_RUNNING_D3D_FULL_SCREEN"),
        s if s == QUNS_BUSY => Some("QUNS_BUSY"),
        s if s == QUNS_PRESENTATION_MODE => Some("QUNS_PRESENTATION_MODE"),
        _ => None,
    }
}
