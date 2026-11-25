//! The uninstall step: close Plith, delete the install, remove the link, the Apps entry and the
//! certificate. The running setup image cannot delete itself, so it, its folder and the root are
//! left for Windows to delete at the next restart.

mod reboot;
#[cfg(test)]
mod tests;

use crate::files::{StepError, audit_root, close_running, remove_all_except};
use crate::processes::Processes;
use crate::register::{Places, unregister};
use plith_host::trust::{PLITH_THUMBPRINT, Scope, Store, remove_cert};
use reboot::{RebootDelete, SystemRebootDelete};
use std::path::{Path, PathBuf};

/// Removes everything the install put on the machine, in `scope`'s certificate stores. Every
/// part that finds nothing is fine, so a rerun after a failure finishes the job.
///
/// # Errors
/// A process could not be closed, the install folder is not safe to delete from (a link, or
/// writable by a non-administrator), or a file, the link, the key or a certificate could not be
/// removed.
pub fn uninstall(
    root: &Path,
    places: &Places,
    scope: Scope,
    procs: &mut dyn Processes,
    log: &mut dyn FnMut(String),
) -> Result<(), StepError> {
    let image = running_image(root);
    let mut restart = Restart {
        image: image.as_deref(),
        delayed: &mut SystemRebootDelete,
    };
    uninstall_with(root, places, scope, procs, log, &mut restart)
}

/// What is left for the next restart: the running setup image (if it lives in the install) and
/// the call that schedules deletes. A fake call is the seam tests use, so nothing real is
/// scheduled.
struct Restart<'a> {
    image: Option<&'a Path>,
    delayed: &'a mut dyn RebootDelete,
}

/// `uninstall` with the restart side given.
fn uninstall_with(
    root: &Path,
    places: &Places,
    scope: Scope,
    procs: &mut dyn Processes,
    log: &mut dyn FnMut(String),
    restart: &mut Restart<'_>,
) -> Result<(), StepError> {
    close_running(root, procs, log).map_err(StepError::new)?;
    if root.exists() {
        log("checking the install folder".to_string());
        audit_root(root)?;
        remove_files(root, restart, log)?;
    }
    log("removing the Start Menu link and the Apps entry".to_string());
    unregister(places)?;
    log("removing the certificate".to_string());
    for store in [Store::Root, Store::TrustedPublisher] {
        remove_cert(scope, store, PLITH_THUMBPRINT)
            .map_err(|reason| StepError::new(format!("cannot remove the certificate: {reason}")))?;
    }
    Ok(())
}

/// Deletes the install except the running image, which is scheduled for the next restart with
/// its folders and the root.
fn remove_files(
    root: &Path,
    restart: &mut Restart<'_>,
    log: &mut dyn FnMut(String),
) -> Result<(), StepError> {
    let kept = restart
        .image
        .map(|image| lowercase(image.strip_prefix(root).unwrap_or(image)));
    remove_all_except(
        root,
        &|path| {
            let relative = path.strip_prefix(root).unwrap_or(path);
            kept.as_deref() == Some(lowercase(relative).as_str())
        },
        log,
    )?;
    let Some(image) = restart.image else {
        return std::fs::remove_dir(root)
            .map_err(|error| StepError::new(format!("cannot remove {}: {error}", root.display())));
    };
    // The file first, then each folder up to the root: Windows deletes in this order.
    let mut doomed: Vec<PathBuf> = vec![image.to_path_buf()];
    let mut folder = image.parent();
    while let Some(dir) = folder {
        doomed.push(dir.to_path_buf());
        if dir == root {
            break;
        }
        folder = dir.parent();
    }
    for path in doomed {
        log(format!("{} goes at the next restart", path.display()));
        restart.delayed.schedule(&path).map_err(StepError::new)?;
    }
    Ok(())
}

fn lowercase(path: &Path) -> String {
    path.to_string_lossy().to_ascii_lowercase()
}

/// The running exe, as a path under `root`, when it lives there. `None` when it runs from
/// elsewhere (a temp copy): then nothing is left behind.
fn running_image(root: &Path) -> Option<PathBuf> {
    let exe = std::fs::canonicalize(std::env::current_exe().ok()?).ok()?;
    let canonical_root = std::fs::canonicalize(root).ok()?;
    let relative = exe.strip_prefix(&canonical_root).ok()?;
    Some(root.join(relative))
}
