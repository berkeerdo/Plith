//! The file step runs against temp folders and a fake `Processes`; nothing here touches
//! `Program Files`, a real process or the registry.

use super::acl::{self, Ace, WRITE_MASK};
use super::pending::{PendingDeletes, drop_pending_under};
use super::{Seams, StepError, install_checked};
use crate::payload::Payload;
use crate::processes::Processes;
use std::fs::{self, OpenOptions};
use std::os::windows::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

const PAYLOAD: Payload = Payload {
    plith: b"plith-bytes",
    helper: b"helper-bytes",
    cert: b"cert-bytes",
};
const SETUP_EXE: &[u8] = b"setup-bytes";

/// A unique temp folder, removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("plith-setup-{name}-{}-{n}", std::process::id()));
        fs::create_dir_all(&path).expect("temp dir");
        Self(path)
    }

    fn root(&self) -> PathBuf {
        self.0.join("Plith")
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Records what the step asked for, and what was on disk at that moment.
#[derive(Default)]
struct FakeProcesses {
    root: PathBuf,
    events: Vec<String>,
    hang: bool,
}

impl FakeProcesses {
    fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            events: Vec::new(),
            hang: false,
        }
    }
}

impl Processes for FakeProcesses {
    fn close_plith(&mut self) -> Result<(), String> {
        let staged = [
            "plith.exe.new",
            "osd\\plith-osd.exe.new",
            "setup\\plith-setup.exe.new",
        ]
        .iter()
        .all(|name| self.root.join(name).exists());
        self.events.push(format!("close staged={staged}"));
        if self.hang {
            return Err("Plith did not close in 5 seconds".to_string());
        }
        Ok(())
    }

    fn stop_by_path(&mut self, exe: &Path) -> Result<(), String> {
        self.events.push(format!("stop {}", exe.display()));
        Ok(())
    }
}

/// An in-memory restart delete list.
#[derive(Default)]
struct FakePending {
    list: Option<Vec<String>>,
    reads: u32,
    writes: Vec<Option<Vec<String>>>,
}

impl PendingDeletes for FakePending {
    fn read(&mut self) -> Result<Option<Vec<String>>, String> {
        self.reads += 1;
        Ok(self.list.clone())
    }

    fn write(&mut self, entries: Option<&[String]>) -> Result<(), String> {
        self.list = entries.map(<[String]>::to_vec);
        self.writes.push(self.list.clone());
        Ok(())
    }
}

/// The install with the ACL check off (a temp root) and a fake restart delete list.
fn install_with(
    root: &Path,
    procs: &mut FakeProcesses,
    pending: &mut FakePending,
    log: &mut dyn FnMut(String),
) -> Result<(), StepError> {
    let mut seams = Seams {
        check_acl: false,
        pending,
    };
    install_checked(root, &PAYLOAD, SETUP_EXE, procs, log, &mut seams)
}

fn run(root: &Path) -> Result<Vec<String>, StepError> {
    let mut procs = FakeProcesses::new(root);
    install_with(root, &mut procs, &mut FakePending::default(), &mut |_| {})?;
    Ok(procs.events)
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_string()).collect()
}

#[test]
fn pending_deletes_under_root_are_dropped() {
    let root = Path::new(r"C:\Program Files\Plith");
    let entries = strings(&[
        r"\??\C:\program files\PLITH\setup\plith-setup.exe",
        "",
        r"\??\C:\Program Files\Plith\old.dll",
        r"\??\C:\Program Files\Plith\new.dll",
        r"\??\C:\Program Files\Other\x.tmp",
        "",
        r"\??\C:\Program Files\Plith2\x",
        "",
    ]);
    assert_eq!(
        drop_pending_under(&entries, root),
        strings(&[
            r"\??\C:\Program Files\Plith\old.dll",
            r"\??\C:\Program Files\Plith\new.dll",
            r"\??\C:\Program Files\Other\x.tmp",
            "",
            r"\??\C:\Program Files\Plith2\x",
            "",
        ])
    );
}

#[test]
fn the_root_itself_is_dropped_and_the_multi_sz_roundtrips() {
    use super::pending::{decode, encode};
    let root = Path::new(r"C:\Program Files\Plith");
    let entries = strings(&[
        r"\??\C:\Program Files\Plith",
        "",
        r"\??\D:\a",
        "!\\??\\D:\\b",
    ]);
    assert_eq!(decode(&encode(&entries)), entries);
    assert_eq!(
        drop_pending_under(&entries, root),
        strings(&[r"\??\D:\a", "!\\??\\D:\\b"])
    );
    assert!(decode(&encode(&[])).is_empty());
}

#[test]
fn pending_list_empty_after_drop_removes_value() {
    let temp = TempDir::new("pending-empty");
    let root = temp.root();
    let mut pending = FakePending {
        list: Some(strings(&[
            &format!(r"\??\{}\setup\plith-setup.exe", root.display()),
            "",
            &format!(r"\??\{}", root.display()),
            "",
        ])),
        ..FakePending::default()
    };
    install_with(
        &root,
        &mut FakeProcesses::new(&root),
        &mut pending,
        &mut |_| {},
    )
    .expect("install");
    assert_eq!(pending.writes, [None], "the value is removed");
}

#[test]
fn other_pending_deletes_stay_and_an_untouched_list_is_not_written() {
    let temp = TempDir::new("pending-other");
    let root = temp.root();
    let other = strings(&[r"\??\C:\elsewhere\x.tmp", ""]);
    let mut pending = FakePending {
        list: Some(other.clone()),
        ..FakePending::default()
    };
    install_with(
        &root,
        &mut FakeProcesses::new(&root),
        &mut pending,
        &mut |_| {},
    )
    .expect("install");
    assert!(pending.writes.is_empty());
    assert_eq!(pending.list, Some(other));
}

#[test]
fn install_cancels_pending_deletes_before_staging() {
    let temp = TempDir::new("pending-order");
    let root = temp.root();
    let mut pending = FakePending {
        list: Some(strings(&[
            &format!(r"\??\{}\setup\plith-setup.exe", root.display()),
            "",
        ])),
        ..FakePending::default()
    };
    let mut log = Vec::new();
    install_with(
        &root,
        &mut FakeProcesses::new(&root),
        &mut pending,
        &mut |line| {
            log.push(line);
        },
    )
    .expect("install");
    assert_eq!(pending.reads, 1);
    let cancel = log
        .iter()
        .position(|line| line.contains("cancelling"))
        .unwrap();
    let first_write = log
        .iter()
        .position(|line| line.starts_with("writing"))
        .unwrap();
    assert!(cancel < first_write, "{log:?}");
}

fn assert_three_files(root: &Path) {
    assert_eq!(fs::read(root.join("plith.exe")).unwrap(), PAYLOAD.plith);
    assert_eq!(
        fs::read(root.join("osd").join("plith-osd.exe")).unwrap(),
        PAYLOAD.helper
    );
    assert_eq!(
        fs::read(root.join("setup").join("plith-setup.exe")).unwrap(),
        SETUP_EXE
    );
}

fn leftover_new_files(root: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "new") {
                found.push(path.display().to_string());
            }
        }
    }
    found
}

#[test]
fn fresh_root_gets_three_files() {
    let temp = TempDir::new("fresh");
    let root = temp.root();
    run(&root).expect("install");
    assert_three_files(&root);
    assert!(leftover_new_files(&root).is_empty());
}

#[test]
fn wpf_files_removed_after_rename() {
    let temp = TempDir::new("wpf");
    let root = temp.root();
    fs::create_dir_all(root.join("runtimes").join("x")).unwrap();
    fs::create_dir_all(root.join("Setup")).unwrap();
    fs::write(root.join("Plith.exe"), b"old wpf exe").unwrap();
    fs::write(root.join("Plith.dll"), b"old dll").unwrap();
    fs::write(root.join("runtimes").join("x").join("y.dll"), b"native").unwrap();
    fs::write(root.join("Setup").join("Plith-Uninstaller.exe"), b"old").unwrap();

    run(&root).expect("install");

    assert_three_files(&root);
    assert!(!root.join("Plith.dll").exists());
    assert!(!root.join("runtimes").exists(), "empty folders go too");
    assert!(!root.join("Setup").join("Plith-Uninstaller.exe").exists());
    assert!(root.join("setup").join("plith-setup.exe").exists());
}

#[test]
fn close_happens_after_new_files_are_written() {
    let temp = TempDir::new("order");
    let root = temp.root();
    let events = run(&root).expect("install");
    assert_eq!(events[0], "close staged=true");
    assert!(events.iter().any(|e| e.ends_with("plith.exe")));
    assert!(events.iter().any(|e| e.ends_with("plith-osd.exe")));
}

#[test]
fn locked_target_fails_naming_file_and_leaves_no_new() {
    let temp = TempDir::new("locked");
    let root = temp.root();
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("plith.exe"), b"running").unwrap();
    let _hold = OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(root.join("plith.exe"))
        .unwrap();

    let error = run(&root).expect_err("a locked target must fail");

    assert!(error.reason.contains("plith.exe"), "{}", error.reason);
    assert!(leftover_new_files(&root).is_empty());
}

#[test]
fn rerun_after_failure_succeeds() {
    let temp = TempDir::new("rerun");
    let root = temp.root();
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("plith.exe"), b"running").unwrap();
    let hold = OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(root.join("plith.exe"))
        .unwrap();
    run(&root).expect_err("locked");
    drop(hold);

    run(&root).expect("rerun");

    assert_three_files(&root);
    assert!(leftover_new_files(&root).is_empty());
}

#[test]
fn junction_inside_root_is_refused_and_target_survives() {
    let temp = TempDir::new("junction");
    let root = temp.root();
    let outside = temp.0.join("outside");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("keep.txt"), b"keep").unwrap();
    fs::create_dir_all(&root).unwrap();
    let status = Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(root.join("link"))
        .arg(&outside)
        .output()
        .expect("mklink");
    assert!(status.status.success(), "mklink failed");

    let error = run(&root).expect_err("a link inside the root is refused");

    assert!(error.reason.contains("link"), "{}", error.reason);
    assert_eq!(fs::read(outside.join("keep.txt")).unwrap(), b"keep");
    assert!(!root.join("plith.exe").exists(), "nothing was written");
    assert!(leftover_new_files(&root).is_empty());
}

#[test]
fn reparse_root_is_refused() {
    let temp = TempDir::new("rootlink");
    let outside = temp.0.join("outside");
    fs::create_dir_all(&outside).unwrap();
    let root = temp.root();
    let status = Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&root)
        .arg(&outside)
        .output()
        .expect("mklink");
    assert!(status.status.success(), "mklink failed");

    let error = run(&root).expect_err("a root that is a link is refused");

    assert!(error.reason.contains("link"), "{}", error.reason);
    assert_eq!(
        fs::read_dir(&outside).unwrap().count(),
        0,
        "nothing written through it"
    );
}

#[test]
fn acl_problem_blocks_staging() {
    let temp = TempDir::new("blocked");
    let root = temp.root();
    fs::create_dir_all(&root).unwrap();
    let status = Command::new("icacls")
        .arg(&root)
        .args(["/grant", "*S-1-5-32-545:(OI)(CI)W"])
        .output()
        .expect("icacls");
    assert!(status.status.success(), "icacls failed");
    let mut procs = FakeProcesses::new(&root);

    let mut pending = FakePending::default();
    let mut seams = Seams {
        check_acl: true,
        pending: &mut pending,
    };
    let error = install_checked(
        &root,
        &PAYLOAD,
        SETUP_EXE,
        &mut procs,
        &mut |_| {},
        &mut seams,
    )
    .expect_err("a Users-writable root must stop the install");

    assert!(error.reason.contains("S-1-5-32-545"), "{}", error.reason);
    assert!(leftover_new_files(&root).is_empty());
    assert!(!root.join("plith.exe").exists());
    assert!(procs.events.is_empty(), "nothing was closed either");
}

#[test]
fn hung_plith_falls_back_to_kill() {
    let temp = TempDir::new("hung");
    let root = temp.root();
    let mut procs = FakeProcesses::new(&root);
    procs.hang = true;

    install_with(&root, &mut procs, &mut FakePending::default(), &mut |_| {}).expect("install");

    let wanted = format!("stop {}", root.join("plith.exe").display());
    assert!(procs.events.contains(&wanted), "{:?}", procs.events);
    assert_three_files(&root);
}

#[test]
fn acl_judgement_flags_users_write_and_passes_admins() {
    let ace = |sid: &str, rights: u32| Ace::Allowed(sid.to_string(), rights);
    let fine = acl::judge(
        Path::new("p"),
        "S-1-5-18",
        &[ace("S-1-5-32-544", 0x1F_01FF)],
    );
    assert!(fine.is_empty(), "{fine:?}");
    let read_only = acl::judge(
        Path::new("p"),
        "S-1-5-18",
        &[ace("S-1-5-32-545", 0x12_00A9)],
    );
    assert!(read_only.is_empty(), "{read_only:?}");
    let bad = acl::judge(
        Path::new("p"),
        "S-1-5-18",
        &[ace("S-1-5-32-545", WRITE_MASK & 0x2)],
    );
    assert_eq!(bad.len(), 1, "{bad:?}");
    let owned = acl::judge(Path::new("p"), "S-1-5-21-1-2-3-1001", &[]);
    assert_eq!(owned.len(), 1, "{owned:?}");
}

#[test]
fn an_ace_type_the_check_cannot_read_is_a_problem() {
    let problems = acl::judge(Path::new("p"), "S-1-5-18", &[Ace::Unknown(4)]);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].contains("unknown ACE type 4"));
}

#[test]
fn acl_check_reads_a_real_folder_that_grants_users_write() {
    let temp = TempDir::new("acl");
    let folder = temp.0.join("granted");
    fs::create_dir_all(&folder).unwrap();
    let status = Command::new("icacls")
        .arg(&folder)
        .args(["/grant", "*S-1-5-32-545:(OI)(CI)W"])
        .output()
        .expect("icacls");
    assert!(status.status.success(), "icacls failed");

    let problems = acl::problems(&folder).expect("read the ACL");

    assert!(
        problems.iter().any(|p| p.contains("S-1-5-32-545")),
        "{problems:?}"
    );
}

#[test]
fn only_paths_under_program_files_are_acl_checked() {
    assert!(acl::under_program_files(Path::new(
        r"C:\Program Files\Plith\plith.exe"
    )));
    assert!(!acl::under_program_files(&std::env::temp_dir()));
    assert!(!acl::under_program_files(Path::new(
        r"C:\Program Files Evil\x"
    )));
}
