//! The hidden-window journal: the handles this process hid, on disk, so a run that ends
//! without any code running (a kill through `TerminateProcess`, an abort) can be cleaned up
//! by the next start.
//!
//! The file (`%LOCALAPPDATA%\Plith\flyout-hidden.txt` in the app) holds one decimal handle per
//! line. It is rewritten through a temp file and a rename, so a crash mid-write leaves either
//! the old or the new list, never a torn one. A missing or unreadable file reads as empty.
//! Free of Win32 types: the window checks arrive as closures.

#[cfg(test)]
mod tests;

use std::fmt::Write as _;
use std::io::{self, ErrorKind};
use std::path::Path;

/// One decimal handle per line, each line ending in `\n`; empty for no handles.
#[must_use]
pub fn format(handles: &[isize]) -> String {
    let mut text = String::with_capacity(handles.len() * 12);
    for handle in handles {
        let _ = writeln!(text, "{handle}");
    }
    text
}

/// The handles in `text`, in order. Blank lines and lines that are not one decimal integer
/// (after trimming whitespace) are skipped.
#[must_use]
pub fn parse(text: &str) -> Vec<isize> {
    text.lines()
        .filter_map(|line| line.trim().parse().ok())
        .collect()
}

/// The handles journaled at `path`; empty when the file is missing or unreadable.
pub(crate) fn read(path: &Path) -> Vec<isize> {
    std::fs::read_to_string(path)
        .map(|text| parse(&text))
        .unwrap_or_default()
}

/// Replaces the journal with `handles` (an empty set removes the file). Writes a sibling
/// `.tmp` file and renames it over the journal, which replaces it atomically on NTFS.
///
/// # Errors
/// The folder, the temp file or the rename failed; the previous journal is left as it was.
pub(crate) fn write(path: &Path, handles: &[isize]) -> io::Result<()> {
    if handles.is_empty() {
        return clear(path);
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let temp = path.with_extension("tmp");
    std::fs::write(&temp, format(handles))?;
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

/// Start-up cleanup after a run that died with windows hidden: calls `show` for every
/// journaled handle `is_orphan` accepts (in the app: the handle is a window, of a host class,
/// and still hidden; a recycled handle that names another window is left alone), then clears
/// the journal. Returns how many were shown.
pub(crate) fn restore_orphans(
    path: &Path,
    is_orphan: impl Fn(isize) -> bool,
    mut show: impl FnMut(isize),
) -> usize {
    let mut shown = 0;
    for handle in read(path) {
        if is_orphan(handle) {
            show(handle);
            shown += 1;
        }
    }
    let _ = clear(path);
    shown
}
