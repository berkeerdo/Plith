//! The hidden-window journal: the handles this process hid, on disk, so a run that ends
//! without any code running (a kill through `TerminateProcess`, an abort) can be cleaned up
//! by the next start.
//!
//! The file (`%LOCALAPPDATA%\Plith\flyout-hidden.txt` in the app) starts with `boot <unix
//! seconds>`, the boot it was written in, then holds one decimal handle per line. Handles do
//! not survive a reboot (and an Explorer restart recreates the flyout), so a journal from
//! another boot, or without a stamp, is discarded unread. The file is rewritten through a temp
//! file and a rename, so a crash mid-write leaves either the old or the new list, never a torn
//! one. A missing or unreadable file reads as empty. Free of Win32 types: the window checks
//! and the clock arrive as arguments.

#[cfg(test)]
mod tests;

use std::fmt::Write as _;
use std::io::{self, ErrorKind};
use std::path::Path;

/// How far two boot stamps may differ and still name the same boot. The stamp is derived
/// from the wall clock minus uptime, so a clock adjustment moves it.
pub const BOOT_TOLERANCE_SECS: u64 = 120;

/// The boot instant in unix seconds from the current unix time and the uptime in ms.
#[must_use]
pub fn boot_stamp(now_unix_secs: u64, uptime_ms: u64) -> u64 {
    now_unix_secs.saturating_sub(uptime_ms / 1000)
}

/// `boot <boot>` on the first line, then one decimal handle per line, each ending in `\n`.
#[must_use]
pub fn format(boot: u64, handles: &[isize]) -> String {
    let mut text = String::with_capacity(16 + handles.len() * 12);
    let _ = writeln!(text, "boot {boot}");
    for handle in handles {
        let _ = writeln!(text, "{handle}");
    }
    text
}

/// The boot stamp (only from a valid `boot <n>` first line) and the handles in `text`, in
/// order. Blank lines and lines that are not one decimal integer (after trimming whitespace)
/// are skipped.
#[must_use]
pub fn parse(text: &str) -> (Option<u64>, Vec<isize>) {
    let mut lines = text.lines().peekable();
    let boot = lines
        .peek()
        .and_then(|line| line.trim().strip_prefix("boot "))
        .and_then(|stamp| stamp.trim().parse().ok());
    if boot.is_some() {
        lines.next();
    }
    let handles = lines.filter_map(|line| line.trim().parse().ok()).collect();
    (boot, handles)
}

/// The handles in `text` when its stamp names the boot `boot` (within
/// [`BOOT_TOLERANCE_SECS`]); empty for another boot or no stamp.
#[must_use]
pub fn handles_for_boot(text: &str, boot: u64) -> Vec<isize> {
    match parse(text) {
        (Some(stamp), handles) if stamp.abs_diff(boot) <= BOOT_TOLERANCE_SECS => handles,
        _ => Vec::new(),
    }
}

/// The handles journaled at `path` in the boot `boot`; empty when the file is missing,
/// unreadable, unstamped or from another boot.
pub(crate) fn read(path: &Path, boot: u64) -> Vec<isize> {
    std::fs::read_to_string(path)
        .map(|text| handles_for_boot(&text, boot))
        .unwrap_or_default()
}

/// Replaces the journal with `handles` (an empty set removes the file). Writes a sibling
/// `.tmp` file and renames it over the journal, which replaces it atomically on NTFS.
///
/// # Errors
/// The folder, the temp file or the rename failed; the previous journal is left as it was.
pub(crate) fn write(path: &Path, boot: u64, handles: &[isize]) -> io::Result<()> {
    if handles.is_empty() {
        return clear(path);
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let temp = path.with_extension("tmp");
    std::fs::write(&temp, format(boot, handles))?;
    std::fs::rename(&temp, path)
}

/// Removes the journal; a journal that does not exist is already clear.
///
/// # Errors
/// The file exists and could not be removed.
pub(crate) fn clear(path: &Path) -> io::Result<()> {
    match std::fs::remove_file(path) {
        Err(err) if err.kind() != ErrorKind::NotFound => Err(err),
        _ => Ok(()),
    }
}

/// Start-up cleanup after a run that died with windows hidden: calls `show` for every handle
/// journaled in this boot that `is_orphan` accepts (in the app [`super::is_orphan`]: still a
/// hidden window with the facts the hide path requires; a recycled handle that names another
/// window is left alone), then clears the journal, also one from another boot. Returns how
/// many were shown.
pub(crate) fn restore_orphans(
    path: &Path,
    boot: u64,
    is_orphan: impl Fn(isize) -> bool,
    mut show: impl FnMut(isize),
) -> usize {
    let mut shown = 0;
    for handle in read(path, boot) {
        if is_orphan(handle) {
            show(handle);
            shown += 1;
        }
    }
    let _ = clear(path);
    shown
}
