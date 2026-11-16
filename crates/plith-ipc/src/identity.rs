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

/// Which pipe server a client may send to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServerTrust {
    /// The installed UIAccess helper: the same user, not in an `AppContainer`, and at an
    /// integrity level strictly above the client's. A same-user process cannot raise its own
    /// level, so a medium squatter is refused. The installed helper was measured at High for
    /// an administrator; a standard user's UIAccess process is expected one step above medium
    /// (not measured here). The default of [`OsdLink::start`](crate::OsdLink::start).
    Installed,
    /// Development only (unsigned helper, tests): the same user, no `AppContainer`, and a
    /// level at least as high as the client's.
    DevSameUser,
}

/// Whether a server with token `server` may receive data from a client with `own`.
pub(crate) fn server_is_trusted(server: &TokenFacts, own: &TokenFacts, trust: ServerTrust) -> bool {
    let level_ok = match trust {
        ServerTrust::Installed => server.integrity_rid > own.integrity_rid,
        ServerTrust::DevSameUser => server.integrity_rid >= own.integrity_rid,
    };
    server.sid == own.sid && !server.app_container && level_ok
}

/// Whether the process serving the open client `pipe` may be trusted under `trust`.
///
/// # Errors
/// [`FactsError`] when the server process or its token cannot be read.
pub(crate) fn server_is_trusted_for(
    pipe: &OwnedHandle,
    trust: ServerTrust,
) -> Result<bool, FactsError> {
    let mut pid = 0u32;
    // SAFETY: the handle is an open pipe client and `pid` is a valid out pointer.
    unsafe { GetNamedPipeServerProcessId(pipe.0, &raw mut pid) }
        .map_err(|e| FactsError::Os(format!("GetNamedPipeServerProcessId: {e}")))?;
    Ok(server_is_trusted(
        &process_token_facts(pid)?,
        &current_token_facts()?,
        trust,
    ))
}
