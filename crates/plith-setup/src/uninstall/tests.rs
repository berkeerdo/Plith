//! The uninstall runs against temp folders, an `HKCU` test key, a `CurrentUser` test certificate
//! store and a fake delete-at-restart; nothing real is scheduled and `Program Files` is untouched.

use super::reboot::RebootDelete;
use super::{Restart, uninstall, uninstall_with};
use crate::register::{read_installed, register};
use crate::testing::{FakeProcesses, TempDir, TestPlaces};
use plith_core::setup::Version;
use plith_host::trust::{
    PLITH_CERT, PLITH_THUMBPRINT, Scope, Store, add_cert, has_cert, remove_cert,
};
use std::path::{Path, PathBuf};

/// Records what would be deleted at the next restart.
#[derive(Default)]
struct FakeReboot(Vec<PathBuf>);

impl RebootDelete for FakeReboot {
    fn schedule(&mut self, path: &Path) -> Result<(), String> {
        self.0.push(path.to_path_buf());
        Ok(())
    }
}

/// Removes the test store's certificate at the end of a test.
struct StoreGuard(Scope);

impl Drop for StoreGuard {
    fn drop(&mut self) {
        let _ = remove_cert(self.0, Store::Root, PLITH_THUMBPRINT);
    }
}

fn installed(name: &'static str, store: &'static str) -> (TempDir, TestPlaces, Scope, StoreGuard) {
    let temp = TempDir::new(name);
    temp.install_three(10);
    let places = TestPlaces::new(&temp, name);
    register(&places.places, &temp.root(), Version::new(0, 4, 0)).expect("register");
    let scope = Scope::CurrentUserTest(store);
    add_cert(scope, Store::Root, PLITH_CERT).expect("add cert");
    (temp, places, scope, StoreGuard(scope))
}

#[test]
fn uninstall_removes_link_key_and_cert() {
    let (temp, places, scope, _guard) = installed("remove", "Plith-test-setup-uninstall-remove");
    // A file that is not ours goes too.
    std::fs::write(temp.root().join("osd").join("stray.dll"), b"x").unwrap();
    let mut procs = FakeProcesses::default();
    uninstall(&temp.root(), &places.places, scope, &mut procs, &mut |_| {}).expect("uninstall");

    assert!(!temp.root().exists(), "the install folder is gone");
    assert!(!places.places.link().exists());
    assert_eq!(read_installed(&places.places), None);
    assert_eq!(has_cert(scope, Store::Root, PLITH_THUMBPRINT), Ok(false));
    assert!(procs.events[0] == "close", "Plith is closed first");
    let sweep = format!("stop under {}", temp.root().display());
    assert_eq!(
        procs.events[1], sweep,
        "everything under the root is stopped"
    );
}

#[test]
fn the_running_setup_image_is_scheduled_for_restart() {
    let (temp, places, scope, _guard) = installed("keep", "Plith-test-setup-uninstall-keep");
    let root = temp.root();
    let image = root.join("setup").join("plith-setup.exe");
    let mut reboot = FakeReboot::default();
    uninstall_with(
        &root,
        &places.places,
        scope,
        &mut FakeProcesses::default(),
        &mut |_| {},
        &mut Restart {
            image: Some(&image),
            delayed: &mut reboot,
        },
    )
    .expect("uninstall");

    assert!(!root.join("plith.exe").exists());
    assert!(!root.join("osd").exists());
    assert!(image.exists(), "the running image stays until the restart");
    assert_eq!(reboot.0, [image, root.join("setup"), root]);
    assert_eq!(read_installed(&places.places), None);
    assert!(!places.places.link().exists());
}

#[test]
fn uninstall_on_nothing_installed_is_ok() {
    let temp = TempDir::new("nothing");
    let places = TestPlaces::new(&temp, "nothing");
    let scope = Scope::CurrentUserTest("Plith-test-setup-uninstall-nothing");
    let mut reboot = FakeReboot::default();
    uninstall_with(
        &temp.root(),
        &places.places,
        scope,
        &mut FakeProcesses::default(),
        &mut |_| {},
        &mut Restart {
            image: None,
            delayed: &mut reboot,
        },
    )
    .expect("nothing to remove is fine");
    assert!(reboot.0.is_empty());
}

#[test]
fn a_link_inside_the_root_fails_closed_and_deletes_nothing() {
    let (temp, places, scope, _guard) =
        installed("uninstall-junction", "Plith-test-setup-uninstall-link");
    let root = temp.root();
    let target = temp.root().parent().unwrap().join("outside");
    std::fs::create_dir_all(&target).unwrap();
    std::fs::write(target.join("precious.txt"), b"keep me").unwrap();
    let junction = root.join("junction");
    let made = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&junction)
        .arg(&target)
        .output()
        .expect("mklink");
    assert!(made.status.success(), "junction created");

    let result = uninstall(
        &root,
        &places.places,
        scope,
        &mut FakeProcesses::default(),
        &mut |_| {},
    );
    assert!(result.is_err_and(|error| error.reason.contains("link")));
    assert!(root.join("plith.exe").exists(), "nothing was deleted");
    assert!(target.join("precious.txt").exists());
    assert_eq!(read_installed(&places.places).as_deref(), Some("0.4.0"));
    let _ = std::fs::remove_dir(&junction);
}
