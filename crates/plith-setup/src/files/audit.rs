//! The look before any write: nothing under the install root may be a link, and (under the real
//! `Program Files`) nothing may be writable by a non-administrator. Elevated writes and deletes
//! follow this check, so a planted link or a swappable folder cannot redirect them.

use super::{StepError, acl};
use std::fs;
use std::os::windows::fs::MetadataExt;
use std::path::Path;

/// `FILE_ATTRIBUTE_REPARSE_POINT`.
const REPARSE_POINT: u32 = 0x400;
/// `FILE_ATTRIBUTE_DIRECTORY`.
const DIRECTORY: u32 = 0x10;

/// Audits `root` and everything under it without following links.
///
/// # Errors
/// `root` or an entry under it is a reparse point, the permissions cannot be read, or (with
/// `check_acl`) a non-administrator can write there.
pub(super) fn audit(root: &Path, check_acl: bool) -> Result<(), StepError> {
    let mut problems = Vec::new();
    visit(root, check_acl, &mut problems)?;
    if problems.is_empty() {
        Ok(())
    } else {
        Err(StepError::new(format!(
            "non-admin write access on the install: {}",
            problems.join("; ")
        )))
    }
}

fn visit(path: &Path, check_acl: bool, problems: &mut Vec<String>) -> Result<(), StepError> {
    let attributes = fs::symlink_metadata(path)
        .map_err(|error| StepError::new(format!("cannot read {}: {error}", path.display())))?
        .file_attributes();
    if attributes & REPARSE_POINT != 0 {
        return Err(StepError::new(format!(
            "{} is a link (reparse point); refusing to install through it",
            path.display()
        )));
    }
    if check_acl {
        problems.extend(acl::problems(path).map_err(StepError::new)?);
    }
    if attributes & DIRECTORY != 0 {
        let entries = fs::read_dir(path)
            .map_err(|error| StepError::new(format!("cannot list {}: {error}", path.display())))?;
        for entry in entries {
            let entry = entry.map_err(|error| {
                StepError::new(format!("cannot list {}: {error}", path.display()))
            })?;
            visit(&entry.path(), check_acl, problems)?;
        }
    }
    Ok(())
}
