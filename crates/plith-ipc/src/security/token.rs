//! The SID of the user that owns this process.

use std::ffi::c_void;

use windows::Win32::Foundation::{HLOCAL, LocalFree};
use windows::Win32::Security::Authorization::ConvertSidToStringSidW;
use windows::Win32::Security::{GetTokenInformation, TOKEN_QUERY, TOKEN_USER, TokenUser};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::core::PWSTR;

use crate::win::OwnedHandle;

/// Returns the string form of the current user's SID, such as `S-1-5-21-...`.
///
/// # Errors
/// Returns the Windows error text when the process token cannot be read.
pub(crate) fn current_user_sid() -> Result<String, String> {
    let mut token = windows::Win32::Foundation::HANDLE::default();
    // SAFETY: `token` is a valid out pointer and the pseudo handle needs no closing.
    unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &raw mut token) }
        .map_err(|e| format!("OpenProcessToken: {e}"))?;
    let token = OwnedHandle(token);

    let mut needed = 0u32;
    // SAFETY: a null buffer with length 0 only asks for the required size.
    let _ = unsafe { GetTokenInformation(token.0, TokenUser, None, 0, &raw mut needed) };
    if needed == 0 {
        return Err("GetTokenInformation reported no size".to_string());
    }
    // u64 elements keep the buffer aligned for TOKEN_USER.
    let mut buffer = vec![0u64; (needed as usize).div_ceil(8)];
    // SAFETY: the buffer is at least `needed` bytes and `needed` is a valid out pointer.
    unsafe {
        GetTokenInformation(
            token.0,
            TokenUser,
            Some(buffer.as_mut_ptr().cast::<c_void>()),
            needed,
            &raw mut needed,
        )
    }
    .map_err(|e| format!("GetTokenInformation: {e}"))?;

    // SAFETY: the call above filled the buffer with a TOKEN_USER and its SID data.
    let sid = unsafe { (*buffer.as_ptr().cast::<TOKEN_USER>()).User.Sid };
    // SAFETY: `sid` points into `buffer`, which outlives the call.
    let mut text = PWSTR::null();
    unsafe { ConvertSidToStringSidW(sid, &raw mut text) }
        .map_err(|e| format!("ConvertSidToStringSidW: {e}"))?;
    // SAFETY: `text` is a valid NUL-terminated string allocated by the call.
    let result = unsafe { text.to_string() }.map_err(|e| e.to_string());
    // SAFETY: the string was allocated with LocalAlloc and is freed exactly once.
    unsafe {
        let _ = LocalFree(Some(HLOCAL(text.0.cast())));
    }
    result
}
