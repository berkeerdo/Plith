//! The register step writes only under an `HKCU` test key, a temp Start Menu folder and a
//! `CurrentUser` test certificate store; the real Apps list and `Program Files` are never touched.

use super::link::read_target;
use super::registry::{create, read_dword, read_string};
use super::{Places, read_installed, register, trust_helper};
use crate::testing::{TempDir, TestPlaces};
use plith_core::setup::Version;
use plith_host::trust::{PLITH_THUMBPRINT, Scope, Store, has_cert, remove_cert};

const VERSION: Version = Version::new(0, 4, 0);

fn string(places: &TestPlaces, name: &str) -> Option<String> {
    read_string(places.places.apps_key.0, places.key(), name)
}

fn dword(places: &TestPlaces, name: &str) -> Option<u32> {
    read_dword(places.places.apps_key.0, places.key(), name)
}

#[test]
fn register_writes_every_value() {
    let temp = TempDir::new("reg-values");
    // 1500 + 6 + 5 bytes is 1511: 2 KB rounded up.
    temp.install_three(1500);
    let root = temp.root();
    let places = TestPlaces::new(&temp, "values");
    register(&places.places, &root, VERSION).expect("register");

    let plith = root.join("plith.exe").display().to_string();
    assert_eq!(string(&places, "DisplayName").as_deref(), Some("Plith"));
    assert_eq!(string(&places, "DisplayVersion").as_deref(), Some("0.4.0"));
    assert_eq!(string(&places, "Publisher").as_deref(), Some("Plith"));
    assert_eq!(string(&places, "DisplayIcon"), Some(format!("{plith},0")));
    assert_eq!(
        string(&places, "InstallLocation"),
        Some(root.display().to_string())
    );
    assert_eq!(
        string(&places, "UninstallString"),
        Some(format!(
            "\"{}\" --uninstall",
            root.join("setup").join("plith-setup.exe").display()
        ))
    );
    assert_eq!(dword(&places, "EstimatedSize"), Some(2));
    assert_eq!(dword(&places, "NoModify"), Some(1));
    assert_eq!(dword(&places, "NoRepair"), Some(1));

    let link = places.places.link();
    assert!(link.is_file(), "the Start Menu link is written");
    assert_eq!(read_target(&link), Ok(plith));
    assert!(
        !link.with_extension("lnk.new").exists(),
        "no staged link is left"
    );
}

#[test]
fn register_replaces_wpf_values() {
    let temp = TempDir::new("reg-wpf");
    temp.install_three(10);
    let places = TestPlaces::new(&temp, "wpf");
    let (hive, path) = (places.places.apps_key.0, places.key().to_string());
    let old = create(hive, &path).expect("seed key");
    for (name, value) in [
        (
            "DisplayVersion",
            "0.3.3+5e2feb9117e9ea0728d44421b8011da98388918a",
        ),
        ("Publisher", "Plith Self-Signed"),
        (
            "UninstallString",
            r#""C:\Program Files\Plith\Setup\Plith-Uninstaller.exe" --uninstall"#,
        ),
        ("DisplayIcon", r"C:\Program Files\Plith\Plith.exe"),
    ] {
        old.set_string(name, value).expect("seed value");
    }
    drop(old);

    register(&places.places, &temp.root(), VERSION).expect("register");

    assert_eq!(string(&places, "DisplayVersion").as_deref(), Some("0.4.0"));
    assert_eq!(string(&places, "Publisher").as_deref(), Some("Plith"));
    let uninstall = string(&places, "UninstallString").expect("uninstall string");
    assert!(uninstall.ends_with(r#"setup\plith-setup.exe" --uninstall"#));
    assert!(!uninstall.contains("Plith-Uninstaller"));
    assert!(string(&places, "DisplayIcon").is_some_and(|icon| icon.ends_with("plith.exe,0")));
}

#[test]
fn register_replaces_an_existing_link() {
    let temp = TempDir::new("reg-link");
    temp.install_three(10);
    let places = TestPlaces::new(&temp, "link");
    std::fs::create_dir_all(&places.places.start_menu).unwrap();
    std::fs::write(places.places.link(), b"old link").unwrap();
    register(&places.places, &temp.root(), VERSION).expect("register");
    assert_eq!(
        read_target(&places.places.link()),
        Ok(temp.root().join("plith.exe").display().to_string())
    );
}

#[test]
fn read_installed_reads_display_version() {
    let temp = TempDir::new("reg-read");
    temp.install_three(10);
    let places = TestPlaces::new(&temp, "read");
    assert_eq!(read_installed(&places.places), None);
    register(&places.places, &temp.root(), VERSION).expect("register");
    assert_eq!(read_installed(&places.places).as_deref(), Some("0.4.0"));
}

#[test]
fn production_places_name_the_real_apps_key_and_link() {
    let places = Places::production().expect("known folder");
    assert_eq!(
        places.apps_key.1,
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Plith"
    );
    assert!(places.link().ends_with("Plith.lnk"));
    assert!(places.start_menu.is_absolute());
}

/// Removes the test store's certificate at the end of a test.
struct StoreGuard(Scope);

impl Drop for StoreGuard {
    fn drop(&mut self) {
        let _ = remove_cert(self.0, Store::Root, PLITH_THUMBPRINT);
    }
}

#[test]
fn trust_helper_adds_the_plith_cert_and_refuses_an_unsigned_helper() {
    let scope = Scope::CurrentUserTest("Plith-test-setup-trust");
    let _guard = StoreGuard(scope);
    let temp = TempDir::new("reg-trust");
    temp.install_three(10);
    let helper = temp.root().join("osd").join("plith-osd.exe");
    let result = trust_helper(scope, &helper);
    assert!(
        result.is_err_and(|error| error.reason.contains("not trusted")),
        "an unsigned file is refused"
    );
    assert_eq!(has_cert(scope, Store::Root, PLITH_THUMBPRINT), Ok(true));
}
