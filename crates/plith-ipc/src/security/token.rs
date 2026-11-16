//! What a process token says about its owner: user SID, integrity level, `AppContainer`.

use std::ffi::c_void;

use windows::Win32::Foundation::{ERROR_ACCESS_DENIED, HANDLE, HLOCAL, LocalFree};
use windows::Win32::Security::Authorization::ConvertSidToStringSidW;
use windows::Win32::Security::{
    GetSidSubAuthority, GetSidSubAuthorityCount, GetTokenInformation, PSID,
    TOKEN_INFORMATION_CLASS, TOKEN_MANDATORY_LABEL, TOKEN_QUERY, TOKEN_USER, TokenIntegrityLevel,
    TokenIsAppContainer, TokenUser,
};
use windows::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::core::PWSTR;

use crate::identity::TokenFacts;
use crate::win::OwnedHandle;

#[cfg(test)]
mod tests;

/// Why a process token could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum FactsError {
    /// The process could not be opened because access was denied.
    AccessDenied,
    /// Any other Windows error, as text.
    Os(String),
}

/// Returns the string form of the current user's SID, such as `S-1-5-21-...`.
///
/// # Errors
/// Returns the Windows error text when the process token cannot be read.
pub(crate) fn current_user_sid() -> Result<String, String> {
    current_token_facts()
        .map(|facts| facts.sid)
        .map_err(|e| format!("{e:?}"))
}

/// Reads the token facts of the current process.
///
/// # Errors
/// Returns [`FactsError`] when the token cannot be read.
pub(crate) fn current_token_facts() -> Result<TokenFacts, FactsError> {
    // SAFETY: the pseudo handle of the current process needs no closing.
    token_facts(unsafe { GetCurrentProcess() })
}

/// Reads the token facts of process `pid`.
///
/// # Errors
/// [`FactsError::AccessDenied`] when the process cannot be opened for a limited query,
/// [`FactsError::Os`] for any other failure.
pub(crate) fn process_token_facts(pid: u32) -> Result<TokenFacts, FactsError> {
    // SAFETY: plain call; the returned handle is owned and closed by the wrapper.
    let process =
        unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.map_err(|e| {
            if e.code() == ERROR_ACCESS_DENIED.to_hresult() {
                FactsError::AccessDenied
            } else {
                FactsError::Os(format!("OpenProcess: {e}"))
            }
        })?;
    let process = OwnedHandle(process);
    token_facts(process.0)
}

fn token_facts(process: HANDLE) -> Result<TokenFacts, FactsError> {
    let mut token = HANDLE::default();
    // SAFETY: `token` is a valid out pointer and `process` is a valid process handle.
    unsafe { OpenProcessToken(process, TOKEN_QUERY, &raw mut token) }
        .map_err(|e| FactsError::Os(format!("OpenProcessToken: {e}")))?;
    let token = OwnedHandle(token);
    let os = FactsError::Os;

    let user = token_info(&token, TokenUser).map_err(os)?;
    // SAFETY: the buffer holds a TOKEN_USER filled by `GetTokenInformation`.
    let sid = unsafe { (*user.as_ptr().cast::<TOKEN_USER>()).User.Sid };
    let sid = sid_to_string(sid).map_err(FactsError::Os)?;

    let label = token_info(&token, TokenIntegrityLevel).map_err(FactsError::Os)?;
    // SAFETY: the buffer holds a TOKEN_MANDATORY_LABEL filled by `GetTokenInformation`.
    let label_sid = unsafe { (*label.as_ptr().cast::<TOKEN_MANDATORY_LABEL>()).Label.Sid };
    // SAFETY: `label_sid` points into `label`, which outlives these calls; the count is
    // at least one for an integrity label, and the last sub-authority is the level.
    let integrity_rid = unsafe {
        let count = u32::from(*GetSidSubAuthorityCount(label_sid));
        if count == 0 {
            return Err(FactsError::Os("empty integrity label".to_string()));
        }
        *GetSidSubAuthority(label_sid, count - 1)
    };

    let container = token_info(&token, TokenIsAppContainer).map_err(FactsError::Os)?;
    let app_container = container
        .first()
        .is_some_and(|word| word.to_le_bytes()[..4] != [0u8; 4]);
    Ok(TokenFacts {
        sid,
        integrity_rid,
        app_container,
    })
}

/// Reads one token information class into an 8-byte aligned buffer.
fn token_info(token: &OwnedHandle, class: TOKEN_INFORMATION_CLASS) -> Result<Vec<u64>, String> {
    let mut needed = 0u32;
    // SAFETY: a null buffer with length 0 only asks for the required size.
    let _ = unsafe { GetTokenInformation(token.0, class, None, 0, &raw mut needed) };
    if needed == 0 {
        return Err("GetTokenInformation reported no size".to_string());
    }
    let mut buffer = vec![0u64; (needed as usize).div_ceil(8)];
    // SAFETY: the buffer is at least `needed` bytes and `needed` is a valid out pointer.
    unsafe {
        GetTokenInformation(
            token.0,
            class,
            Some(buffer.as_mut_ptr().cast::<c_void>()),
            needed,
            &raw mut needed,
        )
    }
    .map_err(|e| format!("GetTokenInformation: {e}"))?;
    Ok(buffer)
}

fn sid_to_string(sid: PSID) -> Result<String, String> {
    let mut text = PWSTR::null();
    // SAFETY: `sid` points into a live token buffer.
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
