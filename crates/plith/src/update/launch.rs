//! Verifies a downloaded setup through one open handle and starts it.

use super::LaunchError;
use plith_core::setup::{Version, quote_arg};
use plith_host::trust::{SignError, verify_plith_handle};
use std::fs::OpenOptions;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::OpenOptionsExt;
use std::path::Path;
use windows::Win32::UI::Shell::{SHELLEXECUTEINFOW, ShellExecuteExW};
use windows::core::{PCWSTR, w};

/// `FILE_SHARE_READ`: while the handle is open nobody can write, rename or delete the file.
const SHARE_READ: u32 = 1;
/// `SW_SHOWNORMAL`.
const SHOW_NORMAL: i32 = 1;

/// Opens `setup` once with share mode read only, checks its signature through that handle, and
/// starts it (verb `open`, so the installer's manifest asks UAC) with `--update --from <own>`
/// while the handle is still open: the file that was judged is the file that runs.
pub(super) fn verify_and_start(setup: &Path, own: Version) -> Result<(), LaunchError> {
    let file = OpenOptions::new()
        .read(true)
        .share_mode(SHARE_READ)
        .open(setup)
        .map_err(|e| LaunchError::Failed(format!("open: {e}")))?;
    match verify_plith_handle(&file, setup) {
        Ok(()) => {}
        Err(SignError::Os(reason)) => return Err(LaunchError::Failed(reason)),
        Err(_) => return Err(LaunchError::BadSignature),
    }
    let result = start(setup, own);
    drop(file);
    result.map_err(LaunchError::Failed)
}

fn start(setup: &Path, own: Version) -> Result<(), String> {
    let file: Vec<u16> = setup
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let args = parameters(own);
    let args: Vec<u16> = args.encode_utf16().chain(std::iter::once(0)).collect();
    let mut info = SHELLEXECUTEINFOW {
        cbSize: u32::try_from(size_of::<SHELLEXECUTEINFOW>()).unwrap_or(0),
        lpVerb: w!("open"),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: PCWSTR(args.as_ptr()),
        nShow: SHOW_NORMAL,
        ..Default::default()
    };
    // SAFETY: `info` is fully initialised; `file` and `args` outlive the call.
    unsafe { ShellExecuteExW(&raw mut info) }.map_err(|e| format!("ShellExecuteExW: {e}"))
}

/// The installer's arguments: `--update --from <own>`.
fn parameters(own: Version) -> String {
    format!("--update --from {}", quote_arg(&own.to_string()))
}

#[cfg(test)]
mod tests {
    use super::parameters;
    use plith_core::setup::Version;

    #[test]
    fn parameters_name_the_old_version() {
        assert_eq!(parameters(Version::new(0, 4, 0)), "--update --from 0.4.0");
    }
}
