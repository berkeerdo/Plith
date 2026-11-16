//! Opening the pipe as a client and checking who serves it.

use windows::Win32::Foundation::GENERIC_WRITE;
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_SHARE_MODE, OPEN_EXISTING, SECURITY_IDENTIFICATION, SECURITY_SQOS_PRESENT,
};
use windows::Win32::System::Pipes::GetNamedPipeServerProcessId;
use windows::core::PCWSTR;

use crate::security::token::{FactsError, current_token_facts, process_token_facts};
use crate::win::OwnedHandle;

/// Opens the pipe for writing at the identification impersonation level, so the server
/// can see who connected but never act as this process.
///
/// `wide_name` must be NUL-terminated.
pub(crate) fn open_client(wide_name: &[u16]) -> windows::core::Result<OwnedHandle> {
    // SAFETY: `wide_name` is NUL-terminated and outlives the call.
    unsafe {
        CreateFileW(
            PCWSTR(wide_name.as_ptr()),
            GENERIC_WRITE.0,
            FILE_SHARE_MODE(0),
            None,
            OPEN_EXISTING,
            SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION,
            None,
        )
    }
    .map(OwnedHandle)
}

/// What decides whether a token can be trusted as the OSD server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TokenFacts {
    /// The user SID as text.
    pub(crate) sid: String,
    /// The integrity level: the last sub-authority of the token's label SID.
    pub(crate) integrity_rid: u32,
    /// Whether the token belongs to an `AppContainer` process.
    pub(crate) app_container: bool,
}

#[cfg(test)]
mod tests;

/// Whether a server with token `server` may receive data from a client with `own`.
///
/// The server must run as the same user, not in an `AppContainer`, and at an integrity
/// level at least as high as ours.
pub(crate) fn server_is_trusted(server: &TokenFacts, own: &TokenFacts) -> bool {
    server.sid == own.sid && !server.app_container && server.integrity_rid >= own.integrity_rid
}

/// Whether the process serving the open client `pipe` may be trusted as the OSD server.
///
/// # Errors
/// [`FactsError`] when the server process or its token cannot be read.
pub(crate) fn server_is_trusted_for(pipe: &OwnedHandle) -> Result<bool, FactsError> {
    let mut pid = 0u32;
    // SAFETY: the handle is an open pipe client and `pid` is a valid out pointer.
    unsafe { GetNamedPipeServerProcessId(pipe.0, &raw mut pid) }
        .map_err(|e| FactsError::Os(format!("GetNamedPipeServerProcessId: {e}")))?;
    Ok(server_is_trusted(
        &process_token_facts(pid)?,
        &current_token_facts()?,
    ))
}
