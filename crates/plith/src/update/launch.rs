//! Verifies a downloaded setup through one open handle and starts exactly that file.

use super::LaunchError;
use plith_core::setup::{Version, quote_arg};
use plith_host::trust::{SignError, verify_plith_handle};
use std::fs::{File, OpenOptions};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::AsRawHandle;
use std::path::Path;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Storage::FileSystem::{
    FILE_NAME_NORMALIZED, GETFINALPATHNAMEBYHANDLE_FLAGS, GetFinalPathNameByHandleW,
    VOLUME_NAME_DOS,
};
use windows::Win32::UI::Shell::{
    SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW,
};
use windows::core::{PCWSTR, w};

/// `FILE_SHARE_READ`: while the handle is open nobody can write, rename or delete the file.
const SHARE_READ: u32 = 1;
/// `SW_SHOWNORMAL`.
const SHOW_NORMAL: i32 = 1;

/// Opens `setup` once with share mode read only, checks its signature through that handle,
/// asks the handle where the file really is, refuses a place other than `setup`, and starts
/// that final path (verb `open`: the installer is `asInvoker`, so it starts at medium and asks
/// UAC later, only for its worker) with `--update --from <own>` while the handle is still open:
/// the file that was judged is the file that runs, even if a folder on the way was turned into a
/// junction.
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
    let actual = final_path(&file).map_err(LaunchError::Failed)?;
    let run = judge_final_path(&actual, setup).map_err(LaunchError::Refused)?;
    let result = start(&run, own);
    drop(file);
    result.map_err(LaunchError::Failed)
}

/// The path Windows reports for the open file, as `\\?\C:\...`.
fn final_path(file: &File) -> Result<String, String> {
    let handle = HANDLE(file.as_raw_handle());
    let mut buffer = vec![0u16; 1024];
    loop {
        // SAFETY: `handle` is open for the whole call and `buffer` is a writable slice.
        let len = unsafe {
            GetFinalPathNameByHandleW(
                handle,
                &mut buffer,
                GETFINALPATHNAMEBYHANDLE_FLAGS(FILE_NAME_NORMALIZED.0 | VOLUME_NAME_DOS.0),
            )
        } as usize;
        if len == 0 {
            return Err(format!(
                "final path: {}",
                windows::core::Error::from_thread()
            ));
        }
        if len < buffer.len() {
            return Ok(String::from_utf16_lossy(&buffer[..len]));
        }
        buffer.resize(len + 1, 0);
    }
}

/// Accepts `actual` (the final path of the held file) only when it is a local drive path equal,
/// ignoring case, to `expected`. Returns the path without the `\\?\` prefix, to launch.
fn judge_final_path(actual: &str, expected: &Path) -> Result<String, String> {
    if actual.starts_with(r"\\?\UNC\") || actual.starts_with(r"\\") && !actual.starts_with(r"\\?\")
    {
        return Err("the setup is on a network path, refused".to_string());
    }
    let plain = actual.strip_prefix(r"\\?\").unwrap_or(actual);
    let wanted = expected.to_string_lossy();
    let wanted = wanted.strip_prefix(r"\\?\").unwrap_or(&wanted);
    if plain.to_lowercase() == wanted.to_lowercase() {
        Ok(plain.to_string())
    } else {
        Err("the setup is not where it was saved, refused".to_string())
    }
}

fn start(setup: &str, own: Version) -> Result<(), String> {
    let file: Vec<u16> = std::ffi::OsStr::new(setup)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let args = parameters(own);
    let args: Vec<u16> = args.encode_utf16().chain(std::iter::once(0)).collect();
    let mut info = SHELLEXECUTEINFOW {
        cbSize: u32::try_from(size_of::<SHELLEXECUTEINFOW>()).unwrap_or(0),
        fMask: SEE_MASK_NOASYNC | SEE_MASK_NOCLOSEPROCESS,
        lpVerb: w!("open"),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: PCWSTR(args.as_ptr()),
        nShow: SHOW_NORMAL,
        ..Default::default()
    };
    // SAFETY: `info` is fully initialised; `file` and `args` outlive the call.
    unsafe { ShellExecuteExW(&raw mut info) }.map_err(|e| format!("ShellExecuteExW: {e}"))?;
    if !info.hProcess.is_invalid() {
        // SAFETY: the process handle is ours (`SEE_MASK_NOCLOSEPROCESS`) and is closed once.
        let _ = unsafe { CloseHandle(info.hProcess) };
    }
    Ok(())
}

/// The installer's arguments: `--update --from <own>`.
fn parameters(own: Version) -> String {
    format!("--update --from {}", quote_arg(&own.to_string()))
}

#[cfg(test)]
mod tests {
    use super::{judge_final_path, parameters};
    use plith_core::setup::Version;
    use std::path::Path;

    #[test]
    fn parameters_name_the_old_version() {
        assert_eq!(parameters(Version::new(0, 4, 0)), "--update --from 0.4.0");
    }

    #[test]
    fn final_path_match_ignores_case_and_the_extended_prefix() {
        let expected = Path::new(r"C:\Users\a\AppData\Local\Plith\updates\Plith-Setup-0.5.0.exe");
        let actual = r"\\?\c:\users\A\appdata\local\plith\UPDATES\plith-setup-0.5.0.exe";
        assert_eq!(
            judge_final_path(actual, expected).unwrap(),
            r"c:\users\A\appdata\local\plith\UPDATES\plith-setup-0.5.0.exe"
        );
    }

    #[test]
    fn final_path_mismatch_is_refused() {
        let expected = Path::new(r"C:\Users\a\AppData\Local\Plith\updates\Plith-Setup-0.5.0.exe");
        assert!(judge_final_path(r"\\?\C:\evil\Plith-Setup-0.5.0.exe", expected).is_err());
        assert!(judge_final_path(r"\\?\UNC\host\share\Plith-Setup-0.5.0.exe", expected).is_err());
        assert!(judge_final_path(r"\\host\share\x.exe", expected).is_err());
    }
}
