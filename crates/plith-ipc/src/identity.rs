//! Opening the pipe as a client and checking who serves it.

use windows::Win32::Foundation::GENERIC_WRITE;
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_SHARE_MODE, OPEN_EXISTING, SECURITY_IDENTIFICATION, SECURITY_SQOS_PRESENT,
};
use windows::Win32::System::Pipes::GetNamedPipeServerProcessId;
use windows::core::PCWSTR;

use crate::security::token::{current_user_sid, process_user_sid};
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

/// Whether the process serving the open client `pipe` runs as the current user.
///
/// # Errors
/// Returns the Windows error text when the server process cannot be identified.
pub(crate) fn server_is_current_user(pipe: &OwnedHandle) -> Result<bool, String> {
    let mut pid = 0u32;
    // SAFETY: the handle is an open pipe client and `pid` is a valid out pointer.
    unsafe { GetNamedPipeServerProcessId(pipe.0, &raw mut pid) }
        .map_err(|e| format!("GetNamedPipeServerProcessId: {e}"))?;
    Ok(process_user_sid(pid)? == current_user_sid()?)
}
