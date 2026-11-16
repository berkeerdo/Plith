//! Starting the OSD helper and naming the pipe of this session.

use std::mem::size_of;
use std::path::Path;

use windows::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::Shell::{SEE_MASK_FLAG_NO_UI, SHELLEXECUTEINFOW, ShellExecuteExW};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::{PCWSTR, w};

use crate::pipe_name;
use crate::win::wide;

/// Returns the pipe path of the OSD helper in this process's Windows session.
///
/// Falls back to session 0 when the session id cannot be read.
#[must_use]
pub fn current_pipe_name() -> String {
    let mut session = 0u32;
    // SAFETY: `session` is a valid out pointer for the duration of the call.
    let found = unsafe { ProcessIdToSessionId(GetCurrentProcessId(), &raw mut session) };
    if found.is_err() {
        session = 0;
    }
    pipe_name(session)
}

/// Starts the OSD helper with the shell `open` verb.
///
/// A plain `CreateProcess` fails on a `uiAccess` executable with
/// `ERROR_ELEVATION_REQUIRED`, so the shell starts it instead.
///
/// # Errors
/// Returns the Windows error text when the helper cannot be started.
pub fn launch_helper(exe: &Path) -> Result<(), String> {
    let file = wide(&exe.to_string_lossy());
    let mut info = SHELLEXECUTEINFOW {
        cbSize: u32::try_from(size_of::<SHELLEXECUTEINFOW>()).unwrap_or(0),
        fMask: SEE_MASK_FLAG_NO_UI,
        lpVerb: w!("open"),
        lpFile: PCWSTR(file.as_ptr()),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    // SAFETY: `info` is fully initialised and `file` outlives the call.
    unsafe { ShellExecuteExW(&raw mut info) }.map_err(|e| format!("ShellExecuteExW: {e}"))
}
