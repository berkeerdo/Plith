//! Process-wide setup.

use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
};

/// Makes the process per-monitor DPI aware (v2). Call once, before creating any window.
///
/// # Errors
/// Returns the Win32 error, for example when the awareness was already set.
pub fn init_process() -> windows::core::Result<()> {
    // SAFETY: the call takes a plain constant and has no pointer arguments.
    unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) }
}
