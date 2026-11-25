//! The helper trust step and the register step: the certificate in the machine stores, the Start
//! Menu link and the Apps list entry.

mod link;
mod registry;
#[cfg(test)]
mod tests;

use crate::files::{INSTALLED_FILES, StepError};
use plith_core::setup::Version;
use plith_host::trust::{PLITH_CERT, Scope, Store, add_cert, verify_plith};
use std::path::{Path, PathBuf};
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::UI::Shell::{FOLDERID_CommonPrograms, KF_FLAG_DEFAULT, SHGetKnownFolderPath};

/// The registry hive an Apps key lives in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hive {
    /// `HKEY_LOCAL_MACHINE`, the real Apps list.
    LocalMachine,
    /// `HKEY_CURRENT_USER`, for tests.
    #[allow(dead_code, reason = "only tests build the per-user hive")]
    CurrentUser,
}

/// Where the register step writes outside the install folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Places {
    /// The Apps entry: hive and key path.
    pub apps_key: (Hive, String),
    /// The folder that holds `Plith.lnk`.
    pub start_menu: PathBuf,
}

/// The Apps entry of the real machine.
const APPS_KEY: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Plith";
/// The Start Menu link name; the WPF app used the same one.
const LINK_NAME: &str = "Plith.lnk";

impl Places {
    /// The real machine: the Apps key in `HKLM` and the all-users Programs folder.
    ///
    /// # Errors
    /// Windows could not name the all-users Programs folder.
    pub fn production() -> Result<Self, StepError> {
        // SAFETY: the returned string is copied and then freed with `CoTaskMemFree` once.
        let folder = unsafe {
            SHGetKnownFolderPath(&FOLDERID_CommonPrograms, KF_FLAG_DEFAULT, None).map(|text| {
                let copy = text.to_string();
                CoTaskMemFree(Some(text.as_ptr().cast()));
                copy
            })
        };
        let folder = folder
            .ok()
            .and_then(Result::ok)
            .filter(|text| !text.is_empty())
            .ok_or_else(|| StepError::new("cannot find the Start Menu folder".to_string()))?;
        Ok(Self {
            apps_key: (Hive::LocalMachine, APPS_KEY.to_string()),
            start_menu: PathBuf::from(folder),
        })
    }

    /// The path of the Start Menu link.
    #[must_use]
    pub fn link(&self) -> PathBuf {
        self.start_menu.join(LINK_NAME)
    }
}

/// Adds the Plith certificate to the Root and Trusted Publishers stores of `scope`, then checks
/// that `helper` carries a valid Plith signature. The certificate is always Plith's own: no
/// caller can hand in another. A file that fails the check leaves the certificate in place.
///
/// # Errors
/// A store could not be written, or `helper` is not signed by Plith.
pub fn trust_helper(scope: Scope, helper: &Path) -> Result<(), StepError> {
    for store in [Store::Root, Store::TrustedPublisher] {
        add_cert(scope, store, PLITH_CERT)
            .map_err(|reason| StepError::new(format!("cannot trust the certificate: {reason}")))?;
    }
    verify_plith(helper)
        .map_err(|error| StepError::new(format!("{} is not trusted: {error}", helper.display())))
}

/// Writes the Start Menu link and the Apps entry for an install at `root`. The existing Apps key
/// is deleted first, so nothing an older app left in it (an old icon or uninstaller) survives.
///
/// # Errors
/// The link or the registry could not be written.
pub fn register(places: &Places, root: &Path, version: Version) -> Result<(), StepError> {
    let plith = root.join(INSTALLED_FILES[0]);
    link::write(&places.link(), &plith, root).map_err(StepError::new)?;
    let (hive, path) = (places.apps_key.0, places.apps_key.1.as_str());
    registry::delete_tree(hive, path).map_err(StepError::new)?;
    let key = registry::create(hive, path).map_err(StepError::new)?;
    let root_text = root.display().to_string();
    let setup = root.join(INSTALLED_FILES[2]);
    let strings = [
        ("DisplayName", "Plith".to_string()),
        ("DisplayVersion", version.to_string()),
        ("Publisher", "Plith".to_string()),
        ("DisplayIcon", format!("{},0", plith.display())),
        ("InstallLocation", root_text),
        (
            "UninstallString",
            format!("\"{}\" --uninstall", setup.display()),
        ),
    ];
    for (name, value) in &strings {
        key.set_string(name, value).map_err(StepError::new)?;
    }
    for (name, value) in [
        ("EstimatedSize", installed_kilobytes(root)),
        ("NoModify", 1),
        ("NoRepair", 1),
    ] {
        key.set_dword(name, value).map_err(StepError::new)?;
    }
    Ok(())
}

/// The size of the installed files in kilobytes, rounded up.
fn installed_kilobytes(root: &Path) -> u32 {
    let bytes: u64 = INSTALLED_FILES
        .iter()
        .filter_map(|relative| std::fs::metadata(root.join(relative)).ok())
        .map(|meta| meta.len())
        .sum();
    u32::try_from(bytes.div_ceil(1024)).unwrap_or(u32::MAX)
}

/// The `DisplayVersion` text of the Apps entry; `None` when there is no entry or no version.
#[must_use]
pub fn read_installed(places: &Places) -> Option<String> {
    registry::read_string(places.apps_key.0, &places.apps_key.1, "DisplayVersion")
}

/// Removes the Start Menu link and the Apps entry; what is not there is fine.
///
/// # Errors
/// The link could not be deleted or the key could not be removed.
pub(crate) fn unregister(places: &Places) -> Result<(), StepError> {
    match std::fs::remove_file(places.link()) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(StepError::new(format!(
                "cannot remove {}: {error}",
                places.link().display()
            )));
        }
    }
    registry::delete_tree(places.apps_key.0, &places.apps_key.1).map_err(StepError::new)
}
