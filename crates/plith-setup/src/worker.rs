//! The elevated worker: the only code that writes Program Files, `HKLM` and the machine
//! certificate stores.
//!
//! It takes nothing from its caller but the job and the pipe name (and `parse_args` has
//! already checked that the name is a setup pipe). The install folder, the files, the
//! certificate and the version all come from this exe. Before it touches anything it makes
//! sure it really is the signed, elevated, release build it should be, so a copy run by hand
//! or a tampered exe does nothing.
//!
//! Exit codes: 0 done, 1 failed (reported), 2 refused (nothing touched, nothing written),
//! 3 another worker holds the lock.
//!
//! Report order: `Step(n)` after step `n` has finished (1 files, 2 trust, 3 register). An
//! uninstall is one step: `Step(3)` after it, so a failure there is `Failed { step: 3 }`.
//! Every failure sends the full English text as `Log` first, then `Failed` with the decimal
//! OS error code (or `"0"`), which the window turns into a localized line.

#[cfg(test)]
mod tests;

use crate::files::{StepError, install_files};
use crate::payload::embedded;
use crate::pipe::report_client;
use crate::processes::SystemProcesses;
use crate::register::{Places, register, trust_helper};
use plith_core::setup::{Job, Report, Version};
use plith_host::trust::{Scope, verify_plith};
use plith_ipc::security::own_integrity_above_medium;
use std::path::PathBuf;
use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE};
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::Shell::{FOLDERID_ProgramFiles, KF_FLAG_DEFAULT, SHGetKnownFolderPath};
use windows::core::w;

/// Exit code: the job is done.
const DONE: i32 = 0;
/// Exit code: a step failed and was reported.
const FAILED: i32 = 1;
/// Exit code: the worker refused to start; it touched nothing.
const REFUSED: i32 = 2;
/// Exit code: another worker is running.
const BUSY: i32 = 3;

/// The install folder: the real Program Files plus `Plith`. Never an argument.
///
/// # Errors
/// Windows could not name the Program Files folder.
pub fn install_root() -> Result<PathBuf, String> {
    // SAFETY: the returned string is copied and then freed with `CoTaskMemFree` once.
    let folder = unsafe {
        SHGetKnownFolderPath(&FOLDERID_ProgramFiles, KF_FLAG_DEFAULT, None).map(|text| {
            let copy = text.to_string();
            CoTaskMemFree(Some(text.as_ptr().cast()));
            copy
        })
    };
    match folder {
        Ok(Ok(text)) if !text.is_empty() => Ok(PathBuf::from(text).join("Plith")),
        _ => Err("cannot find the Program Files folder".to_string()),
    }
}

/// Holds the machine-wide worker lock until dropped.
struct WorkerLock(HANDLE);

impl WorkerLock {
    /// `None` when another worker holds the lock or it cannot be made.
    fn take() -> Option<Self> {
        // SAFETY: the name is a static NUL-terminated string; no attributes are passed.
        let handle = unsafe { CreateMutexW(None, false, w!("Global\\Plith.Setup.Worker")) }.ok()?;
        // SAFETY: reads this thread's last error, set by the call above.
        let existed = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
        let lock = Self(handle);
        (!existed).then_some(lock)
    }
}

impl Drop for WorkerLock {
    fn drop(&mut self) {
        // SAFETY: the handle is owned by this value and closed exactly once.
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

/// Runs the job and returns the exit code. Everything the worker may do starts here.
#[must_use]
pub fn run(job: Job, pipe: &str) -> i32 {
    let Some(exe) = may_run() else {
        return REFUSED;
    };
    let Some(_lock) = WorkerLock::take() else {
        return BUSY;
    };
    // Without a pipe the install still runs; it only loses its progress.
    let client = report_client(pipe).ok();
    let send = |report: Report| {
        if let Some(client) = &client {
            let _ = client.send(&report);
        }
    };
    let outcome = match job {
        Job::Install => install(&exe, &send),
        Job::Uninstall => remove(&send),
    };
    match outcome {
        Ok(()) => {
            send(Report::Finished);
            DONE
        }
        Err((step, error)) => {
            send(Report::Log(error.reason.clone()));
            send(Report::Failed {
                step,
                reason: os_code(&error.reason).to_string(),
            });
            FAILED
        }
    }
}

/// The path of this exe when the worker is allowed to run: elevated, signed by Plith and a
/// release build with a real payload. Reads only; nothing is written.
fn may_run() -> Option<PathBuf> {
    if !matches!(own_integrity_above_medium(), Ok(true)) {
        return None;
    }
    let exe = std::env::current_exe().ok()?;
    verify_plith(&exe).ok()?;
    embedded().is_release().then_some(exe)
}

fn own_version() -> Result<Version, StepError> {
    Version::parse(env!("CARGO_PKG_VERSION"))
        .ok_or_else(|| StepError::new("this exe has no valid version".to_string()))
}

/// Steps 1 to 3 of an install; the error carries the step that failed.
fn install(exe: &std::path::Path, send: &dyn Fn(Report)) -> Result<(), (u8, StepError)> {
    let root = install_root().map_err(|reason| (1, StepError::new(reason)))?;
    let mut log = |text: String| send(Report::Log(text));
    let setup_exe = std::fs::read(exe)
        .map_err(|error| (1, StepError::new(format!("cannot read this exe: {error}"))))?;
    install_files(
        &root,
        &embedded(),
        &setup_exe,
        &mut SystemProcesses,
        &mut log,
    )
    .map_err(|error| (1, error))?;
    send(Report::Step(1));
    trust_helper(Scope::LocalMachine, &root.join("osd").join("plith-osd.exe"))
        .map_err(|error| (2, error))?;
    send(Report::Step(2));
    let places = Places::production().map_err(|error| (3, error))?;
    let version = own_version().map_err(|error| (3, error))?;
    register(&places, &root, version).map_err(|error| (3, error))?;
    send(Report::Step(3));
    Ok(())
}

/// The uninstall: one step, reported as step 3.
fn remove(send: &dyn Fn(Report)) -> Result<(), (u8, StepError)> {
    let root = install_root().map_err(|reason| (3, StepError::new(reason)))?;
    let places = Places::production().map_err(|error| (3, error))?;
    let mut log = |text: String| send(Report::Log(text));
    crate::uninstall::uninstall(
        &root,
        &places,
        Scope::LocalMachine,
        &mut SystemProcesses,
        &mut log,
    )
    .map_err(|error| (3, error))?;
    send(Report::Step(3));
    Ok(())
}

/// The OS error code in a failure text, in decimal; `"0"` when the text carries none.
///
/// The file, trust and register steps fail with English text only, built from `io::Error`
/// (`... (os error 5)`) or a Windows error (`... (0x80070005)`); the last such mark wins.
pub(crate) fn os_code(reason: &str) -> u32 {
    let mut found = 0;
    let mut rest = reason;
    while let Some(open) = rest.find('(') {
        rest = &rest[open + 1..];
        let Some(close) = rest.find(')') else { break };
        let inner = &rest[..close];
        if let Some(digits) = inner.strip_prefix("os error ") {
            if let Ok(code) = digits.parse::<u32>() {
                found = code;
            }
        } else if let Some(hex) = inner.strip_prefix("0x")
            && let Ok(bits) = u32::from_str_radix(hex, 16)
        {
            found = if bits & 0xFFFF_0000 == 0x8007_0000 {
                bits & 0xFFFF
            } else {
                bits
            };
        }
    }
    found
}
