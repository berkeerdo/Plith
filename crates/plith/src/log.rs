//! The log file `%LOCALAPPDATA%\Plith\plith.log`: one timestamped line per call, append only.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender, SyncSender};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::System::SystemInformation::GetLocalTime;
use windows::Win32::UI::Shell::{FOLDERID_LocalAppData, KF_FLAG_DEFAULT, SHGetKnownFolderPath};

/// A local time of day, split into the fields the line prefix needs.
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

/// Formats a stamp as `YYYY-MM-DD HH:MM:SS.mmm`.
#[must_use]
pub fn format_stamp(s: Stamp) -> String {
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}",
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

/// `%LOCALAPPDATA%\Plith\plith.log` from the known-folder API.
fn default_path() -> io::Result<PathBuf> {
    // SAFETY: the folder id is a static GUID; no token asks for the current user.
    let raw = unsafe { SHGetKnownFolderPath(&FOLDERID_LocalAppData, KF_FLAG_DEFAULT, None) }
        .map_err(io::Error::other)?;
    // SAFETY: `raw` is a NUL-terminated wide string that the call allocated for us.
    let folder = unsafe { raw.to_string() };
    // SAFETY: the shell allocated `raw` with the COM task allocator; it is freed once, after
    // the last read above.
    unsafe { CoTaskMemFree(Some(raw.0.cast_const().cast())) };
    let folder = folder.map_err(io::Error::other)?;
    Ok(PathBuf::from(folder).join("Plith").join("plith.log"))
}

/// How long [`LogFile::flush`] waits for the writer before giving up.
const FLUSH_WAIT: Duration = Duration::from_secs(1);

/// What the writer thread receives.
enum Message {
    /// A formatted line to append.
    Line(String),
    /// Acknowledge once everything queued before this marker is written.
    Flush(SyncSender<()>),
}

/// An append-only log file that any thread may write to. A `plith-log` thread owns the file, so
/// a caller (the render thread included) never waits on disk I/O. Dropping the log closes the
/// channel and joins the writer: every line sent before the drop is on disk afterwards.
pub struct LogFile {
    /// `Option` so `drop` can close the channel before joining. `Sender` is `Sync`.
    lines: Option<Sender<Message>>,
    writer: Option<JoinHandle<()>>,
}

impl LogFile {
    /// Opens the log at its fixed place, creating the folder when needed.
    ///
    /// # Errors
    /// Returns the I/O error when the folder or file cannot be created.
    pub fn open() -> io::Result<Self> {
        Self::open_at(&default_path()?)
    }

    /// Opens (or creates) the log at `path`, appending to what is there.
    ///
    /// # Errors
    /// Returns the I/O error when the file cannot be opened.
    pub fn open_at(path: &Path) -> io::Result<Self> {
        if let Some(folder) = path.parent() {
            fs::create_dir_all(folder)?;
        }
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        let (lines, received) = mpsc::channel::<Message>();
        let writer = thread::Builder::new()
            .name(String::from("plith-log"))
            .spawn(move || {
                for message in received {
                    match message {
                        Message::Line(line) => {
                            let _ = file.write_all(line.as_bytes());
                        }
                        Message::Flush(ack) => {
                            let _ = file.flush();
                            let _ = ack.send(());
                        }
                    }
                }
            })?;
        Ok(Self {
            lines: Some(lines),
            writer: Some(writer),
        })
    }

    /// Queues `text` with the current local time in front (taken on the calling thread, so the
    /// stamp is the moment of the call). Write errors are ignored, and so is a line sent after
    /// the writer has ended.
    pub fn line(&self, text: &str) {
        let line = format!("{} {text}\n", format_stamp(now()));
        if let Some(lines) = &self.lines {
            let _ = lines.send(Message::Line(line));
        }
    }

    /// Blocks until every line queued before this call is written, for at most one second (a
    /// dead writer must not hang an exit). Call it before `std::process::exit`, which skips
    /// destructors and so would lose the queue.
    pub fn flush(&self) {
        let Some(lines) = &self.lines else { return };
        let (ack, acked) = mpsc::sync_channel(1);
        if lines.send(Message::Flush(ack)).is_ok() {
            let _ = acked.recv_timeout(FLUSH_WAIT);
        }
    }
}

impl Drop for LogFile {
    fn drop(&mut self) {
        // Closing the channel ends the writer's loop once it has written what is queued.
        drop(self.lines.take());
        if let Some(writer) = self.writer.take() {
            let _ = writer.join();
        }
    }
}

#[cfg(test)]
mod tests;
