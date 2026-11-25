//! Helpers shared by the tests of the register and uninstall steps.

use crate::processes::Processes;
use crate::register::{Hive, Places};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RegDeleteTreeW};
use windows::core::{HSTRING, PCWSTR};

/// A unique temp folder, removed on drop.
pub struct TempDir(PathBuf);

impl TempDir {
    pub fn new(name: &str) -> Self {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("plith-setup-{name}-{}-{n}", std::process::id()));
        fs::create_dir_all(&path).expect("temp dir");
        Self(path)
    }

    pub fn root(&self) -> PathBuf {
        self.0.join("Plith")
    }

    /// Writes the three installed files under the root.
    pub fn install_three(&self, plith_size: usize) {
        let root = self.root();
        fs::create_dir_all(root.join("osd")).unwrap();
        fs::create_dir_all(root.join("setup")).unwrap();
        fs::write(root.join("plith.exe"), vec![1u8; plith_size]).unwrap();
        fs::write(root.join("osd").join("plith-osd.exe"), b"helper").unwrap();
        fs::write(root.join("setup").join("plith-setup.exe"), b"setup").unwrap();
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A test Apps key under `HKCU` and a Start Menu folder inside a temp dir; the key tree is
/// deleted on drop, also when an assertion fails.
pub struct TestPlaces {
    pub places: Places,
}

impl TestPlaces {
    pub fn new(temp: &TempDir, name: &str) -> Self {
        let key = format!(r"Software\Plith-test-setup-{name}-{}", std::process::id());
        Self {
            places: Places {
                apps_key: (Hive::CurrentUser, key),
                start_menu: temp.0.join("Programs"),
            },
        }
    }

    pub fn key(&self) -> &str {
        &self.places.apps_key.1
    }
}

impl Drop for TestPlaces {
    fn drop(&mut self) {
        let wide = HSTRING::from(self.key());
        // SAFETY: a NUL-terminated wide key name of this test, under the current user.
        let _ = unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, PCWSTR(wide.as_ptr())) };
    }
}

/// Records what a step asked for; nothing real is closed or stopped.
#[derive(Default)]
pub struct FakeProcesses {
    pub events: Vec<String>,
}

impl Processes for FakeProcesses {
    fn close_plith(&mut self) -> Result<(), String> {
        self.events.push("close".to_string());
        Ok(())
    }

    fn stop_under(&mut self, root: &Path) -> Result<(), String> {
        self.events.push(format!("stop under {}", root.display()));
        Ok(())
    }
}
