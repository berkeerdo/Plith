//! Raw handle values and the window handle type.

use windows::Win32::Foundation::HWND;

/// The `HWND` for a handle value that was stored or sent across threads as an integer.
pub(crate) fn hwnd_of(raw: isize) -> HWND {
    HWND(raw as *mut core::ffi::c_void)
}
