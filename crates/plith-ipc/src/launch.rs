//! Starting the OSD helper and naming the pipe of this session.

use std::mem::size_of;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, GetLastError, WAIT_TIMEOUT};
use windows::Win32::System::Pipes::WaitNamedPipeW;
use windows::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows::Win32::System::Threading::{GetCurrentProcessId, GetProcessId, WaitForSingleObject};
use windows::Win32::UI::Shell::{
    SEE_MASK_FLAG_NO_UI, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW,
    ShellExecuteExW,
};
use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;
use windows::core::{PCWSTR, w};

use crate::pipe_name;
use crate::security::token::current_user_sid;
use crate::win::OwnedHandle;

/// Where `scripts/install-osd.ps1` installs the signed helper. Only an exe under a trusted
/// folder like this one gets UIAccess.
pub const INSTALLED_HELPER: &str = r"C:\Program Files\Plith\osd\plith-osd.exe";

/// Returns the pipe path of the OSD helper for this process's user and Windows session.
///
/// Falls back to session 0 when the session id cannot be read, and to the word `nosid` when
/// the token cannot be read (the notch and its helper run as the same user, so both fall
/// back the same way).
#[must_use]
pub fn current_pipe_name() -> String {
    let mut session = 0u32;
    // SAFETY: `session` is a valid out pointer for the duration of the call.
    let found = unsafe { ProcessIdToSessionId(GetCurrentProcessId(), &raw mut session) };
    if found.is_err() {
        session = 0;
    }
    let sid = current_user_sid().unwrap_or_else(|_| String::from("nosid"));
    pipe_name(session, &sid)
}

/// True while any server instance of the pipe `name` exists, busy or not. Never connects:
/// `WaitNamedPipeW` returns at once with `ERROR_FILE_NOT_FOUND` when no instance exists, and
/// otherwise waits at most 1 ms (a busy instance answers with a timeout, which still exists).
#[must_use]
pub fn pipe_exists(name: &str) -> bool {
    let wide = crate::win::wide(name);
    // SAFETY: `wide` is NUL-terminated and outlives the call.
    if unsafe { WaitNamedPipeW(PCWSTR(wide.as_ptr()), 1) }.as_bool() {
        return true;
    }
    // SAFETY: reads this thread's last error, set by the call above.
    let error = unsafe { GetLastError() };
    error != ERROR_FILE_NOT_FOUND
}

/// A started helper process. Its handle is closed on drop; dropping it does not stop the
/// helper (the helper ends itself after `Bye` or when the notch goes away).
pub struct HelperProcess {
    pid: u32,
    handle: OwnedHandle,
}

impl HelperProcess {
    /// Wraps an owned process handle; `None` for a null handle.
    fn from_handle(handle: OwnedHandle) -> Option<Self> {
        if handle.0.is_invalid() {
            return None;
        }
        // SAFETY: the handle is a live process handle owned by `handle`.
        let pid = unsafe { GetProcessId(handle.0) };
        Some(Self { pid, handle })
    }

    /// The helper's process id (0 when Windows did not report it).
    #[must_use]
    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// Whether the helper has ended, so the notch can tell a dead helper from one that is
    /// still starting or busy. Never blocks (a zero-timeout wait). A failed wait counts as
    /// ended, so the caller relaunches rather than waits forever.
    #[must_use]
    pub fn has_exited(&self) -> bool {
        // SAFETY: the handle is a live process handle owned by this value.
        let wait = unsafe { WaitForSingleObject(self.handle.0, 0) };
        wait != WAIT_TIMEOUT
    }
}

/// Starts the OSD helper with the shell `open` verb and returns its process.
///
/// A plain `CreateProcess` fails on a `uiAccess` executable with
/// `ERROR_ELEVATION_REQUIRED`, so the shell starts it instead. The helper is started hidden
/// and never activated (`SW_HIDE`; it shows its own window without activation).
///
/// Threading: the call waits until the start is done (`SEE_MASK_NOASYNC`), so any thread
/// may call it, also one that ends right after. The Shell documentation asks for COM to be
/// initialized on the calling thread (`CoInitializeEx`, apartment threaded) because a verb
/// can be handled by a COM shell extension; call it from such a thread when one is at hand.
///
/// # Errors
/// Returns the Windows error text when the helper cannot be started, or when the shell
/// reports success without a process handle.
pub fn launch_helper(exe: &Path) -> Result<HelperProcess, String> {
    let file: Vec<u16> = exe
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut info = SHELLEXECUTEINFOW {
        cbSize: u32::try_from(size_of::<SHELLEXECUTEINFOW>()).unwrap_or(0),
        fMask: SEE_MASK_FLAG_NO_UI | SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC,
        lpVerb: w!("open"),
        lpFile: PCWSTR(file.as_ptr()),
        nShow: SW_HIDE.0,
        ..Default::default()
    };
    // SAFETY: `info` is fully initialised and `file` outlives the call.
    unsafe { ShellExecuteExW(&raw mut info) }.map_err(|e| format!("ShellExecuteExW: {e}"))?;
    // The process handle is ours to close from here on, null or not.
    HelperProcess::from_handle(OwnedHandle(info.hProcess))
        .ok_or_else(|| String::from("ShellExecuteExW: no process handle"))
}

#[cfg(test)]
mod tests;
