//! The log file `%LOCALAPPDATA%\Plith\plith-osd.log`: one timestamped line per call, append only.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use windows::Win32::System::SystemInformation::GetLocalTime;

/// A local time, split into the fields the line prefix needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stamp {
    /// Year, four digits.
    pub year: u16,
    /// Month, 1 to 12.
    pub month: u16,
    /// Day of the month.
    pub day: u16,
    /// Hour, 0 to 23.
    pub hour: u16,
    /// Minute.
    pub minute: u16,
    /// Second.
    pub second: u16,
    /// Millisecond.
    pub millis: u16,
}

/// Formats one log line: `YYYY-MM-DD HH:MM:SS.mmm <text>` and a newline.
#[must_use]
pub fn format_line(s: Stamp, text: &str) -> String {
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03} {text}\n",
        s.year, s.month, s.day, s.hour, s.minute, s.second, s.millis
    )
}

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

fn open_file() -> Option<File> {
    let folder = PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join("Plith");
    fs::create_dir_all(&folder).ok()?;
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(folder.join("plith-osd.log"))
        .ok()
}

/// An append-only log that any thread may write to. Every I/O failure is ignored: without a
/// file the log drops its lines.
pub struct Log {
    file: Mutex<Option<File>>,
}

impl Log {
    /// Opens the log at its fixed place, creating the folder when needed. Never fails; when the
    /// file cannot be opened the log has no file.
    #[must_use]
    pub fn open() -> Self {
        Self {
            file: Mutex::new(open_file()),
        }
    }

    /// A log without a file: every line is dropped (tests).
    #[cfg(test)]
    #[must_use]
    pub fn closed() -> Self {
        Self {
            file: Mutex::new(None),
        }
    }

    /// Appends `text` with the current local time in front.
    pub fn line(&self, text: &str) {
        let line = format_line(now(), text);
        let mut file = match self.file.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some(file) = file.as_mut() {
            let _ = file.write_all(line.as_bytes());
        }
    }
}

#[cfg(test)]
mod tests;
