//! The SID of a user, read from a process token.

use std::ffi::c_void;

use windows::Win32::Foundation::{HANDLE, HLOCAL, LocalFree};
use windows::Win32::Security::Authorization::ConvertSidToStringSidW;
use windows::Win32::Security::{GetTokenInformation, TOKEN_QUERY, TOKEN_USER, TokenUser};
use windows::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::core::PWSTR;

use crate::win::OwnedHandle;

#[cfg(test)]
mod tests;

/// Returns the string form of the current user's SID, such as `S-1-5-21-...`.
///
/// # Errors
/// Returns the Windows error text when the process token cannot be read.
pub(crate) fn current_user_sid() -> Result<String, String> {
    // SAFETY: the pseudo handle of the current process needs no closing.
    token_user_sid(unsafe { GetCurrentProcess() })
}

/// Returns the string form of the SID of the user that owns process `pid`.
///
/// # Errors
/// Returns the Windows error text when the process or its token cannot be opened.
pub(crate) fn process_user_sid(pid: u32) -> Result<String, String> {
    // SAFETY: plain call; the returned handle is owned and closed by the wrapper.
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }
        .map_err(|e| format!("OpenProcess: {e}"))?;
    let process = OwnedHandle(process);
    token_user_sid(process.0)
}

fn token_user_sid(process: HANDLE) -> Result<String, String> {
    let mut token = HANDLE::default();
    // SAFETY: `token` is a valid out pointer and `process` is a valid process handle.
    unsafe { OpenProcessToken(process, TOKEN_QUERY, &raw mut token) }
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
    let mut text = PWSTR::null();
    // SAFETY: `sid` points into `buffer`, which outlives the call.
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
