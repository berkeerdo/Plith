//! The play log file `%LOCALAPPDATA%\Plith\plays.txt`. The text form lives in `plith-core`.

use plith_core::plays::PlayLog;
use plith_host::log::{self, LogFile};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The play log file next to the log.
///
/// # Errors
/// Returns the error of the known-folder call.
pub fn default_path() -> io::Result<PathBuf> {
    Ok(log::data_dir()?.join("plays.txt"))
}

/// The play log in the file at `path`; empty when there is none or it cannot be read.
#[must_use]
pub fn load_at(path: &Path) -> PlayLog {
    fs::read_to_string(path).map_or_else(|_| PlayLog::default(), |text| PlayLog::parse(&text))
}

/// Writes the play log to `path`, creating the folder. The text goes to `plays.txt.tmp` first
/// and is renamed over the old file, so a crash mid-write leaves the old log, never a cut one.
/// It stays on the calling (UI) thread: a play closing is one small write, a few hundred bytes
/// a day, and a saver thread would add a lifetime to manage for no measurable gain.
///
/// # Errors
/// Returns the I/O error when the folder or the file cannot be written.
pub fn save_at(path: &Path, plays: &PlayLog) -> io::Result<()> {
    if let Some(folder) = path.parent() {
        fs::create_dir_all(folder)?;
    }
    let mut temp = path.as_os_str().to_owned();
    temp.push(".tmp");
    let temp = PathBuf::from(temp);
    fs::write(&temp, plays.serialize())?;
    fs::rename(&temp, path)
}

/// The saved play log, or an empty one.
#[must_use]
pub fn load(log: &LogFile) -> PlayLog {
    match default_path() {
        Ok(path) => load_at(&path),
        Err(error) => {
            log.line(&format!("plays: no folder: {error}"));
            PlayLog::default()
        }
    }
}

/// A saver that writes to the default file and logs a failure.
#[must_use]
pub fn saver(log: std::sync::Arc<LogFile>) -> Box<dyn Fn(&PlayLog)> {
    Box::new(move |plays| {
        let saved = default_path().and_then(|path| save_at(&path, plays));
        if let Err(error) = saved {
            log.line(&format!("plays: cannot save: {error}"));
        }
    })
}

#[cfg(test)]
mod tests;
