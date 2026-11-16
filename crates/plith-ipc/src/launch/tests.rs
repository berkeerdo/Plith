use super::*;

use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
};

/// Wraps a process the test can open, the way `launch_helper` wraps the shell's handle.
fn open(pid: u32) -> HelperProcess {
    // SAFETY: plain open of a process this user owns; the handle is owned by the result.
    let handle = unsafe {
        OpenProcess(
            PROCESS_SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION,
            false,
            pid,
        )
    }
    .expect("open process");
    HelperProcess::from_handle(OwnedHandle(handle)).expect("a process handle")
}

#[test]
fn a_running_process_has_not_exited() {
    let me = open(std::process::id());
    assert_eq!(me.pid(), std::process::id());
    assert!(!me.has_exited());
}

#[test]
fn an_ended_process_has_exited() {
    let mut child = std::process::Command::new(r"C:\Windows\System32\cmd.exe")
        .args(["/d", "/c", "exit 0"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("spawn cmd");
    // Opened before the wait, so the pid cannot be reused while the test holds it.
    let process = open(child.id());
    child.wait().expect("wait");
    assert_eq!(process.pid(), child.id());
    assert!(process.has_exited());
}

#[test]
fn the_installed_helper_path_matches_the_install_script() {
    assert_eq!(
        INSTALLED_HELPER,
        r"C:\Program Files\Plith\osd\plith-osd.exe"
    );
}
