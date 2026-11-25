//! The file step of an install: put the three files in place, remove everything else of the old
//! app, and check that only administrators can change what was installed.
//!
//! Order: write every `.new`, close the running app, rename each `.new` over its target, remove
//! what is not ours, check the ACLs. A failure before the renames leaves the old install whole;
//! a rerun starts clean because no `.new` is left behind.

mod acl;
mod audit;
#[cfg(test)]
mod tests;
mod walk;

use crate::payload::Payload;
use crate::processes::Processes;
use std::fmt;
use std::path::{Path, PathBuf};

/// A step failed; `reason` is for the log and the failure screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepError {
    /// What went wrong, naming the file.
    pub reason: String,
}

impl fmt::Display for StepError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.reason)
    }
}

impl StepError {
    pub(crate) fn new(reason: String) -> Self {
        Self { reason }
    }
}

/// The three installed files, relative to the root: the app, the helper, the setup copy.
pub(crate) const INSTALLED_FILES: [&str; 3] =
    ["plith.exe", "osd\\plith-osd.exe", "setup\\plith-setup.exe"];

/// One installed file: its path relative to the root and its bytes.
struct Item<'a> {
    relative: &'static str,
    bytes: &'a [u8],
}

/// Installs the three files under `root`.
///
/// `setup_exe` is the caller's own bytes, written as `setup\plith-setup.exe`.
///
/// # Errors
/// A file could not be written or replaced, a process could not be closed, a file that is not
/// ours could not be removed, or the install is writable by someone other than administrators.
pub fn install_files(
    root: &Path,
    payload: &Payload,
    setup_exe: &[u8],
    procs: &mut dyn Processes,
    log: &mut dyn FnMut(String),
) -> Result<(), StepError> {
    let check_acl = acl::under_program_files(root);
    install_checked(root, payload, setup_exe, procs, log, check_acl)
}

/// `install_files` with the ACL check forced on or off: the seam tests use, because the real
/// answer depends on where `root` is.
fn install_checked(
    root: &Path,
    payload: &Payload,
    setup_exe: &[u8],
    procs: &mut dyn Processes,
    log: &mut dyn FnMut(String),
    check_acl: bool,
) -> Result<(), StepError> {
    // Look before any write: a missing root is created (inside Program Files it inherits the
    // admin-only ACL) and then audited like an existing one; links and, when checked, non-admin
    // write access fail closed with nothing written.
    log("checking the install folder".to_string());
    if !root.exists() {
        std::fs::create_dir_all(root).map_err(|error| {
            StepError::new(format!("cannot create {}: {error}", root.display()))
        })?;
    }
    audit::audit(root, check_acl)?;
    let items = [
        Item {
            relative: INSTALLED_FILES[0],
            bytes: payload.plith,
        },
        Item {
            relative: INSTALLED_FILES[1],
            bytes: payload.helper,
        },
        Item {
            relative: INSTALLED_FILES[2],
            bytes: setup_exe,
        },
    ];
    let staged = stage(root, &items, log)?;
    if let Err(reason) = close_running(root, procs, log) {
        remove_staged(&staged);
        return Err(StepError::new(reason));
    }
    swap_in(root, &items, &staged, log)?;
    log("removing files that are not part of the app".to_string());
    walk::remove_not_ours(root, log)?;
    check_acls(check_acl, root, &items, log)
}

/// The look before any write, for a step that works on an existing root: refuses links and,
/// under the real `Program Files`, non-administrator write access.
///
/// # Errors
/// `root` or something under it is a link or writable by a non-administrator, or cannot be read.
pub(crate) fn audit_root(root: &Path) -> Result<(), StepError> {
    audit::audit(root, acl::under_program_files(root))
}

/// Deletes everything under `root` (never following links) except the files `keep` names.
///
/// # Errors
/// An entry could not be listed or deleted.
pub(crate) fn remove_all_except(
    root: &Path,
    keep: &dyn Fn(&Path) -> bool,
    log: &mut dyn FnMut(String),
) -> Result<(), StepError> {
    walk::remove_except(root, keep, log)
}

/// The `.new` path of an item.
fn staged_path(root: &Path, item: &Item<'_>) -> PathBuf {
    root.join(format!("{}.new", item.relative))
}

/// Writes every `.new`. On a failure the ones written so far are deleted first.
fn stage(
    root: &Path,
    items: &[Item<'_>],
    log: &mut dyn FnMut(String),
) -> Result<Vec<PathBuf>, StepError> {
    let mut staged = Vec::new();
    for item in items {
        let path = staged_path(root, item);
        log(format!("writing {}", path.display()));
        let written = path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(&path, item.bytes));
        // Count the file before checking: a failed write can still leave a partial file.
        staged.push(path.clone());
        if let Err(error) = written {
            remove_staged(&staged);
            return Err(StepError::new(format!(
                "cannot write {}: {error}",
                path.display()
            )));
        }
    }
    Ok(staged)
}

/// Deletes `.new` files; a file that is already gone is fine.
fn remove_staged(staged: &[PathBuf]) {
    for path in staged {
        let _ = std::fs::remove_file(path);
    }
}

/// Closes the running app, then ends anything still running from the two exes' own paths.
pub(crate) fn close_running(
    root: &Path,
    procs: &mut dyn Processes,
    log: &mut dyn FnMut(String),
) -> Result<(), String> {
    log("closing Plith".to_string());
    // A Plith that ignores the close request is ended by its exact path below.
    if let Err(reason) = procs.close_plith() {
        log(format!("Plith did not close politely: {reason}"));
    }
    for relative in &INSTALLED_FILES[..2] {
        let exe = root.join(relative);
        log(format!("stopping {}", exe.display()));
        procs.stop_by_path(&exe)?;
    }
    Ok(())
}

/// Renames each `.new` over its target. If one fails, the file is named, the `.new` files not
/// yet renamed are deleted and the files already renamed stay (a rerun repairs).
fn swap_in(
    root: &Path,
    items: &[Item<'_>],
    staged: &[PathBuf],
    log: &mut dyn FnMut(String),
) -> Result<(), StepError> {
    for (index, item) in items.iter().enumerate() {
        let target = root.join(item.relative);
        log(format!("replacing {}", target.display()));
        if let Err(error) = std::fs::rename(&staged[index], &target) {
            remove_staged(&staged[index..]);
            return Err(StepError::new(format!(
                "cannot replace {}: {error}",
                target.display()
            )));
        }
    }
    Ok(())
}

/// Fails when anything installed can be changed by someone who is not an administrator. Only
/// applies under the real `Program Files`; a test folder elsewhere is not checked.
fn check_acls(
    check_acl: bool,
    root: &Path,
    items: &[Item<'_>],
    log: &mut dyn FnMut(String),
) -> Result<(), StepError> {
    if !check_acl {
        return Ok(());
    }
    log("checking permissions".to_string());
    let mut paths = vec![root.to_path_buf()];
    for item in items {
        let file = root.join(item.relative);
        if let Some(parent) = file.parent().filter(|parent| *parent != root) {
            paths.push(parent.to_path_buf());
        }
        paths.push(file);
    }
    let mut problems = Vec::new();
    for path in &paths {
        problems.extend(acl::problems(path).map_err(StepError::new)?);
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(StepError::new(format!(
            "non-admin write access on the install: {}",
            problems.join("; ")
        )))
    }
}
