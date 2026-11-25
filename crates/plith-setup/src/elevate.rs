//! Starting the elevated worker: this same exe, through the shell `runas` verb.

#[cfg(test)]
mod tests;

use plith_core::setup::{Job, quote_arg};
use std::fmt;
use std::mem::size_of;
use std::os::windows::ffi::OsStrExt;
use windows::Win32::Foundation::{CloseHandle, ERROR_CANCELLED, HANDLE, WAIT_TIMEOUT};
use windows::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject};
use windows::Win32::UI::Shell::{
    SEE_MASK_FLAG_NO_UI, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW,
    ShellExecuteExW,
};
use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;
use windows::core::{PCWSTR, w};

/// Why the worker could not be started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElevateError {
    /// The user said no in the UAC prompt (`ERROR_CANCELLED`, 1223).
    Cancelled,
    /// Anything else, as English text for the log.
    Os(String),
}

impl fmt::Display for ElevateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled => f.write_str("the elevation prompt was cancelled"),
            Self::Os(text) => f.write_str(text),
        }
    }
}

/// The Win32 error behind an `HRESULT` of the `FACILITY_WIN32` kind (`0x8007xxxx`).
fn win32_code(hresult: i32) -> Option<u32> {
    let bits = hresult.cast_unsigned();
    (bits & 0xFFFF_0000 == 0x8007_0000).then_some(bits & 0xFFFF)
}

/// Maps a failed `ShellExecuteExW` to the error the window acts on.
pub(crate) fn classify(hresult: i32, text: &str) -> ElevateError {
    if win32_code(hresult) == Some(ERROR_CANCELLED.0) {
        ElevateError::Cancelled
    } else {
        ElevateError::Os(format!("ShellExecuteExW: {text}"))
    }
}

/// The started worker. The handle is closed on drop; dropping does not stop the worker.
pub struct WorkerProcess(HANDLE);

// SAFETY: a process handle is a process-wide token that any thread may use or close.
unsafe impl Send for WorkerProcess {}

impl WorkerProcess {
    /// The exit code once the worker has ended; `None` while it runs. Never blocks.
    ///
    /// The worker exits with 0 (done), 1 (failed), 2 (refused to start) or 3 (another worker
    /// runs), so the window can tell what happened when no report arrived.
    #[must_use]
    pub fn exit_code(&self) -> Option<u32> {
        // SAFETY: the handle is a live process handle owned by this value.
        if unsafe { WaitForSingleObject(self.0, 0) } == WAIT_TIMEOUT {
            return None;
        }
        let mut code = 0u32;
        // SAFETY: `code` is a valid out pointer for the duration of the call.
        unsafe { GetExitCodeProcess(self.0, &raw mut code) }
            .is_ok()
            .then_some(code)
    }
}

impl Drop for WorkerProcess {
    fn drop(&mut self) {
        // SAFETY: the handle is owned by this value and closed exactly once.
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

/// The command line of the worker: only the job word and the pipe name, both quoted.
pub(crate) fn worker_parameters(job: Job, pipe: &str) -> String {
    let word = match job {
        Job::Install => "install",
        Job::Uninstall => "uninstall",
    };
    format!("--work {} {}", quote_arg(word), quote_arg(pipe))
}

/// Starts this exe elevated as the worker for `job` and returns once the shell has started it
/// (or the user has refused).
///
/// # Errors
/// [`ElevateError::Cancelled`] when the user declines the prompt, [`ElevateError::Os`] for
/// anything else, including a shell that reports success without a process handle.
pub fn start_worker(job: Job, pipe: &str) -> Result<WorkerProcess, ElevateError> {
    let exe = std::env::current_exe()
        .map_err(|error| ElevateError::Os(format!("cannot find this exe: {error}")))?;
    let file: Vec<u16> = exe
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let parameters: Vec<u16> = worker_parameters(job, pipe)
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut info = SHELLEXECUTEINFOW {
        cbSize: u32::try_from(size_of::<SHELLEXECUTEINFOW>()).unwrap_or(0),
        fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI,
        lpVerb: w!("runas"),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: PCWSTR(parameters.as_ptr()),
        nShow: SW_HIDE.0,
        ..Default::default()
    };
    // SAFETY: `info` is fully initialised; `file` and `parameters` outlive the call.
    unsafe { ShellExecuteExW(&raw mut info) }
        .map_err(|error| classify(error.code().0, &error.to_string()))?;
    if info.hProcess.is_invalid() {
        return Err(ElevateError::Os(
            "ShellExecuteExW: no process handle".to_string(),
        ));
    }
    Ok(WorkerProcess(info.hProcess))
}
