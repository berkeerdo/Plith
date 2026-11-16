//! The log file `%LOCALAPPDATA%\Plith\plith-osd.log`: one timestamped line per call, append only.
//!
//! The helper runs at High integrity while the folder is writable at medium, so the file is
//! opened with care: the folder comes from `SHGetKnownFolderPath`, not the environment; a
//! folder or file that is a reparse point is refused; and a handle whose final path is not the
//! expected one (a junction swapped in on the way) is dropped. Every line has its control
//! characters escaped, and a run appends at most [`rules::LOG_CAP`] bytes; an older log past
//! that size is moved to `plith-osd.old.log` at open. The pure rules live in [`rules`].

pub mod rules;

pub use rules::Stamp;

use rules::{CAP_NOTICE, LineFate, format_line, line_fate, rotate_at_open, same_final_path};
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::windows::ffi::OsStringExt;
use std::os::windows::fs::MetadataExt;
use std::os::windows::io::AsRawHandle;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Storage::FileSystem::{FILE_NAME_NORMALIZED, GetFinalPathNameByHandleW};
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::System::SystemInformation::GetLocalTime;
use windows::Win32::UI::Shell::{FOLDERID_LocalAppData, KF_FLAG_DEFAULT, SHGetKnownFolderPath};

/// `FILE_ATTRIBUTE_REPARSE_POINT`.
const REPARSE_POINT: u32 = 0x400;

fn now() -> Stamp {
    // SAFETY: the call has no arguments and returns the time by value.
    let t = unsafe { GetLocalTime() };
    Stamp {
        year: t.wYear,
        month: t.wMonth,
        day: t.wDay,
        hour: t.wHour,
        minute: t.wMinute,
        second: t.wSecond,
        millis: t.wMilliseconds,
    }
}

/// `%LOCALAPPDATA%` of the process token, never the environment variable.
fn local_app_data() -> Option<PathBuf> {
    // SAFETY: a valid known folder id, default flags and the current user's token.
    let raw =
        unsafe { SHGetKnownFolderPath(&FOLDERID_LocalAppData, KF_FLAG_DEFAULT, None) }.ok()?;
    // SAFETY: on success `raw` is a NUL terminated string the shell allocated; it is copied
    // and then freed exactly once with the COM allocator, as the API requires.
    let folder = unsafe {
        let folder = OsString::from_wide(raw.as_wide());
        CoTaskMemFree(Some(raw.0 as *const _));
        folder
    };
    Some(PathBuf::from(folder))
}

/// True when `path` exists and is a reparse point (junction, symbolic link, mount point).
fn is_reparse_point(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|meta| meta.file_attributes() & REPARSE_POINT != 0)
}

/// The path Windows resolved for an open file, as `\\?\C:\...`.
fn final_path(file: &File) -> Option<String> {
    let mut buffer = vec![0u16; 1024];
    let handle = HANDLE(file.as_raw_handle());
    // SAFETY: the handle is open for the duration of the call and the buffer is writable.
    let len = unsafe { GetFinalPathNameByHandleW(handle, &mut buffer, FILE_NAME_NORMALIZED) };
    let len = usize::try_from(len).ok()?;
    (len > 0 && len < buffer.len()).then(|| String::from_utf16_lossy(&buffer[..len]))
}

/// Opens the log for appending and returns it with its current size, or `None` when any
/// check fails (the log then drops its lines).
fn open_file() -> Option<(File, u64)> {
    let folder = local_app_data()?.join("Plith");
    if is_reparse_point(&folder) {
        return None;
    }
    fs::create_dir_all(&folder).ok()?;
    let path = folder.join("plith-osd.log");
    if is_reparse_point(&path) {
        return None;
    }
    if fs::metadata(&path).is_ok_and(|meta| rotate_at_open(meta.len())) {
        let old = folder.join("plith-osd.old.log");
        if !is_reparse_point(&old) {
            let _ = fs::rename(&path, old);
        }
    }
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .ok()?;
    if !same_final_path(&final_path(&file)?, &path.to_string_lossy()) {
        return None;
    }
    let size = file.metadata().map_or(0, |meta| meta.len());
    Some((file, size))
}

/// The open file and what this run has appended.
struct Sink {
    file: File,
    written: u64,
    capped: bool,
}

/// An append-only log that any thread may write to. Every I/O failure is ignored: without a
/// file the log drops its lines.
pub struct Log {
    sink: Mutex<Option<Sink>>,
}

impl Log {
    /// Opens the log at its fixed place, creating the folder when needed. Never fails; when the
    /// file cannot be opened safely the log has no file.
    #[must_use]
    pub fn open() -> Self {
        Self {
            sink: Mutex::new(open_file().map(|(file, _)| Sink {
                file,
                written: 0,
                capped: false,
            })),
        }
    }

    /// A log without a file: every line is dropped (tests).
    #[cfg(test)]
    #[must_use]
    pub fn closed() -> Self {
        Self {
            sink: Mutex::new(None),
        }
    }

    /// Appends `text` (control characters escaped) with the current local time in front.
    pub fn line(&self, text: &str) {
        let stamp = now();
        let line = format_line(stamp, text);
        let mut sink = match self.sink.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let Some(sink) = sink.as_mut() else {
            return;
        };
        let bytes = match line_fate(sink.written, line.len() as u64, sink.capped) {
            LineFate::Write => line,
            LineFate::Notice => {
                sink.capped = true;
                format_line(stamp, CAP_NOTICE)
            }
            LineFate::Drop => return,
        };
        if sink.file.write_all(bytes.as_bytes()).is_ok() {
            sink.written += bytes.len() as u64;
        }
    }
}

#[cfg(test)]
mod tests;
