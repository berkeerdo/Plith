//! Removing everything under the install root that is not one of our three files.
//!
//! The walk never follows a reparse point (junction or symlink): the link itself is deleted and
//! its target is never opened, so a link planted in the install folder cannot steer the delete
//! elsewhere. A folder is held open (without delete sharing) while it is walked.

use super::StepError;
use plith_core::setup::is_ours;
use std::fs;
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
use std::path::Path;

/// `FILE_ATTRIBUTE_REPARSE_POINT`.
const REPARSE_POINT: u32 = 0x400;
/// `FILE_SHARE_READ | FILE_SHARE_WRITE`: no `FILE_SHARE_DELETE`.
const FILE_SHARE_READ_WRITE: u32 = 0x3;
/// Needed to open a directory.
const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
/// Open a link itself instead of following it.
const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
/// `FILE_ATTRIBUTE_DIRECTORY`.
const DIRECTORY: u32 = 0x10;

/// Deletes every file and folder under `root` that `is_ours` does not allow, then the folders
/// left empty, bottom-up.
///
/// # Errors
/// An entry could not be listed or deleted; the reason names it.
pub(super) fn remove_not_ours(root: &Path, log: &mut dyn FnMut(String)) -> Result<(), StepError> {
    remove_except(root, &|path| is_ours(&relative(root, path)), log)
}

/// Deletes every file and folder under `root` except the files `keep` names, then the folders
/// left empty, bottom-up; a folder that still holds a kept file stays. `root` itself stays.
///
/// # Errors
/// An entry could not be listed or deleted; the reason names it.
pub(super) fn remove_except(
    root: &Path,
    keep: &dyn Fn(&Path) -> bool,
    log: &mut dyn FnMut(String),
) -> Result<(), StepError> {
    walk(keep, root, log)
}

fn walk(
    keep: &dyn Fn(&Path) -> bool,
    dir: &Path,
    log: &mut dyn FnMut(String),
) -> Result<(), StepError> {
    let entries = fs::read_dir(dir)
        .map_err(|error| StepError::new(format!("cannot list {}: {error}", dir.display())))?;
    for entry in entries {
        let entry = entry
            .map_err(|error| StepError::new(format!("cannot list {}: {error}", dir.display())))?;
        let path = entry.path();
        let attributes = fs::symlink_metadata(&path)
            .map_err(|error| StepError::new(format!("cannot read {}: {error}", path.display())))?
            .file_attributes();
        let is_dir = attributes & DIRECTORY != 0;
        if attributes & REPARSE_POINT != 0 {
            log(format!("removing link {}", path.display()));
            // A directory link (junction or directory symlink) goes with `remove_dir`, which
            // removes the link and never the target's contents.
            let removed = if is_dir {
                fs::remove_dir(&path)
            } else {
                fs::remove_file(&path)
            };
            removed.map_err(|error| fail_remove(&path, &error))?;
        } else if is_dir {
            // Held open without delete sharing while it is walked, so it cannot be renamed or
            // replaced by a link in between; the attributes are read from the handle itself.
            let guard = open_dir(&path)?;
            let swapped = guard
                .metadata()
                .map_err(|error| {
                    StepError::new(format!("cannot read {}: {error}", path.display()))
                })?
                .file_attributes()
                & REPARSE_POINT
                != 0;
            if swapped {
                return Err(StepError::new(format!(
                    "{} turned into a link while the install ran",
                    path.display()
                )));
            }
            walk(keep, &path, log)?;
            drop(guard);
            if is_empty(&path) {
                log(format!("removing folder {}", path.display()));
                fs::remove_dir(&path).map_err(|error| fail_remove(&path, &error))?;
            }
        } else if !keep(&path) {
            log(format!("removing {}", path.display()));
            remove_file(&path).map_err(|error| fail_remove(&path, &error))?;
        }
    }
    Ok(())
}

/// Opens a directory itself, never what a link there points at, and shares it for reading and
/// writing but not for delete or rename.
fn open_dir(path: &Path) -> Result<fs::File, StepError> {
    fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ_WRITE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
        .map_err(|error| StepError::new(format!("cannot open {}: {error}", path.display())))
}

/// The path under `root`, with backslashes, as `is_ours` expects.
fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

fn is_empty(dir: &Path) -> bool {
    fs::read_dir(dir).is_ok_and(|mut entries| entries.next().is_none())
}

/// Deletes a file, clearing a read-only flag when that is what stands in the way.
fn remove_file(path: &Path) -> std::io::Result<()> {
    fs::remove_file(path).or_else(|first| {
        let mut permissions = fs::metadata(path)?.permissions();
        if !permissions.readonly() {
            return Err(first);
        }
        #[allow(
            clippy::permissions_set_readonly_false,
            reason = "Windows only: this clears the read-only attribute"
        )]
        permissions.set_readonly(false);
        fs::set_permissions(path, permissions)?;
        fs::remove_file(path)
    })
}

fn fail_remove(path: &Path, error: &std::io::Error) -> StepError {
    StepError::new(format!("cannot remove {}: {error}", path.display()))
}
