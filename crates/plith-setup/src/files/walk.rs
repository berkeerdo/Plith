//! Removing everything under the install root that is not one of our three files.
//!
//! The walk never follows a reparse point (junction or symlink): the link itself is deleted and
//! its target is never opened, so a link planted in the install folder cannot steer the delete
//! elsewhere.

use super::StepError;
use plith_core::setup::is_ours;
use std::fs;
use std::os::windows::fs::MetadataExt;
use std::path::Path;

/// `FILE_ATTRIBUTE_REPARSE_POINT`.
const REPARSE_POINT: u32 = 0x400;
/// `FILE_ATTRIBUTE_DIRECTORY`.
const DIRECTORY: u32 = 0x10;

/// Deletes every file and folder under `root` that `is_ours` does not allow, then the folders
/// left empty, bottom-up. A missing root is fine.
///
/// # Errors
/// An entry could not be listed or deleted; the reason names it.
pub(super) fn remove_not_ours(root: &Path, log: &mut dyn FnMut(String)) -> Result<(), StepError> {
    walk(root, root, log)
}

fn walk(root: &Path, dir: &Path, log: &mut dyn FnMut(String)) -> Result<(), StepError> {
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
            walk(root, &path, log)?;
            if is_empty(&path) {
                log(format!("removing folder {}", path.display()));
                fs::remove_dir(&path).map_err(|error| fail_remove(&path, &error))?;
            }
        } else if !is_ours(&relative(root, &path)) {
            log(format!("removing {}", path.display()));
            remove_file(&path).map_err(|error| fail_remove(&path, &error))?;
        }
    }
    Ok(())
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
