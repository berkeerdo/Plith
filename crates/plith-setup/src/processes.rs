//! Closing a running Plith before its files are replaced.

use std::path::Path;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{
    CloseHandle, HANDLE, LPARAM, LUID, WAIT_OBJECT_0, WAIT_TIMEOUT, WPARAM,
};
use windows::Win32::Security::{
    AdjustTokenPrivileges, LUID_AND_ATTRIBUTES, LookupPrivilegeValueW, SE_DEBUG_NAME,
    SE_PRIVILEGE_ENABLED, TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE, PROCESS_TERMINATE,
    QueryFullProcessImageNameW, TerminateProcess, WaitForSingleObject,
};
use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, IsWindow, PostMessageW, WM_CLOSE};
use windows::core::{PWSTR, w};

/// How long each close or stop may take.
const WAIT: Duration = Duration::from_secs(5);
/// How often the wait loop looks again.
const POLL: Duration = Duration::from_millis(50);

/// What the file step needs from the running system. A fake stands in for it in tests.
pub trait Processes {
    /// Asks a running Plith to close and waits for its window to go away.
    ///
    /// # Errors
    /// The window is still there after the wait.
    fn close_plith(&mut self) -> Result<(), String>;

    /// Ends every process whose image is exactly `exe` and waits for it to exit.
    ///
    /// # Errors
    /// A process could not be ended or did not exit in time.
    fn stop_by_path(&mut self, exe: &Path) -> Result<(), String>;
}

/// The real thing.
#[derive(Debug, Default)]
pub struct SystemProcesses;

impl Processes for SystemProcesses {
    fn close_plith(&mut self) -> Result<(), String> {
        // SAFETY: plain window lookups and a posted message; no pointers are held.
        unsafe {
            let Ok(window) = FindWindowW(w!("Plith.Notch"), None) else {
                return Ok(());
            };
            PostMessageW(Some(window), WM_CLOSE, WPARAM::default(), LPARAM::default())
                .map_err(|error| format!("cannot ask Plith to close: {error}"))?;
            let deadline = Instant::now() + WAIT;
            while IsWindow(Some(window)).as_bool() {
                if Instant::now() >= deadline {
                    return Err("Plith did not close in 5 seconds".to_string());
                }
                std::thread::sleep(POLL);
            }
        }
        Ok(())
    }

    fn stop_by_path(&mut self, exe: &Path) -> Result<(), String> {
        enable_debug_privilege();
        let wanted = exe.to_string_lossy().into_owned();
        let mut failures = Vec::new();
        for pid in all_pids() {
            if let Err(error) = stop_pid(pid, &wanted) {
                failures.push(error);
            }
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(failures.join("; "))
        }
    }
}

/// Closes a handle when dropped.
struct Handle(HANDLE);

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: the handle was opened by this process and is closed once.
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

/// Turns `SeDebugPrivilege` on for this process so protected processes can be opened. Failure
/// is ignored: an unelevated token cannot have it, and the open then fails for those alone.
fn enable_debug_privilege() {
    // SAFETY: the token handle is closed by `Handle`; the structures outlive the calls.
    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
            &raw mut token,
        )
        .is_err()
        {
            return;
        }
        let token = Handle(token);
        let mut luid = LUID::default();
        if LookupPrivilegeValueW(None, SE_DEBUG_NAME, &raw mut luid).is_err() {
            return;
        }
        let privileges = TOKEN_PRIVILEGES {
            PrivilegeCount: 1,
            Privileges: [LUID_AND_ATTRIBUTES {
                Luid: luid,
                Attributes: SE_PRIVILEGE_ENABLED,
            }],
        };
        let _ = AdjustTokenPrivileges(token.0, false, Some(&raw const privileges), 0, None, None);
    }
}

/// The ids of every running process.
fn all_pids() -> Vec<u32> {
    let mut pids = Vec::new();
    // SAFETY: the snapshot handle is closed by `Handle`; `entry` is sized before each call.
    unsafe {
        let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            return pids;
        };
        let snapshot = Handle(snapshot);
        let mut entry = PROCESSENTRY32W {
            dwSize: u32::try_from(size_of::<PROCESSENTRY32W>()).unwrap_or(0),
            ..Default::default()
        };
        let mut more = Process32FirstW(snapshot.0, &raw mut entry).is_ok();
        while more {
            pids.push(entry.th32ProcessID);
            more = Process32NextW(snapshot.0, &raw mut entry).is_ok();
        }
    }
    pids
}

/// Ends the process `pid` when its image is exactly `wanted`, on ONE handle: the path check, the
/// kill and the wait all use it, so a reused id cannot point them at another process. A process
/// that is not ours to open, has another path or is already exiting is left alone.
fn stop_pid(pid: u32, wanted: &str) -> Result<(), String> {
    // SAFETY: the handle is closed by `Handle`; the buffer and its length match.
    unsafe {
        let Ok(opened) = OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE | PROCESS_SYNCHRONIZE,
            false,
            pid,
        ) else {
            return Ok(());
        };
        let process = Handle(opened);
        let mut buffer = vec![0u16; 32_768];
        let mut len = u32::try_from(buffer.len()).unwrap_or(0);
        if QueryFullProcessImageNameW(
            process.0,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &raw mut len,
        )
        .is_err()
        {
            return Ok(());
        }
        if !String::from_utf16_lossy(&buffer[..len as usize]).eq_ignore_ascii_case(wanted) {
            return Ok(());
        }
        // Already signaled: it is exiting (a Plith that just got WM_CLOSE); nothing to kill.
        let exited = || WaitForSingleObject(process.0, 0) == WAIT_OBJECT_0;
        if exited() {
            return Ok(());
        }
        if let Err(error) = TerminateProcess(process.0, 1) {
            // It may have exited between the check and the kill.
            if exited() {
                return Ok(());
            }
            return Err(format!("cannot end process {pid}: {error}"));
        }
        if WaitForSingleObject(process.0, 5_000) == WAIT_TIMEOUT {
            return Err(format!("process {pid} did not exit in 5 seconds"));
        }
    }
    Ok(())
}
