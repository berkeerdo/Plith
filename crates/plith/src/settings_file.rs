//! The settings file `%LOCALAPPDATA%\Plith\settings.ini`: read at start, written when missing.
//! The format lives in `plith-core`; this only does the file and the log lines.

use plith_core::settings::{Settings, parse, serialize};
use plith_host::log::{self, LogFile};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The settings file next to the log.
///
/// # Errors
/// Returns the error of the known-folder call.
pub fn default_path() -> io::Result<PathBuf> {
    Ok(log::data_dir()?.join("settings.ini"))
}

/// The settings from the default file; defaults when the folder cannot be found.
#[must_use]
pub fn load(log: &LogFile) -> Settings {
    match default_path() {
        Ok(path) => load_at(&path, log),
        Err(error) => {
            log.line(&format!("settings: no folder, defaults used: {error}"));
            Settings::default()
        }
    }
}

/// The settings in the file at `path`. A missing file gives the defaults and writes one; a bad
/// line is logged and skipped. Never fails: an unreadable file also gives the defaults.
#[must_use]
pub fn load_at(path: &Path, log: &LogFile) -> Settings {
    match fs::read_to_string(path) {
        Ok(text) => {
            let (settings, problems) = parse(&text);
            for problem in problems {
                log.line(&format!("settings: {problem}"));
            }
            settings
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let settings = Settings::default();
            match save_at(path, &settings) {
                Ok(()) => log.line("settings: no file, wrote the defaults"),
                Err(error) => log.line(&format!("settings: cannot write the defaults: {error}")),
            }
            settings
        }
        Err(error) => {
            log.line(&format!("settings: cannot read the file: {error}"));
            Settings::default()
        }
    }
}

/// Writes `settings` to `path` through `settings.ini.tmp` and a rename, so a crash never leaves
/// a half-written file. Creates the folder. Line breaks in the city would split its
/// line in two, so they are dropped first.
///
/// # Errors
/// Returns the I/O error when the folder or the file cannot be written.
pub fn save_at(path: &Path, settings: &Settings) -> io::Result<()> {
    if let Some(folder) = path.parent() {
        fs::create_dir_all(folder)?;
    }
    let clean = Settings {
        city: settings.city.replace(['\r', '\n'], ""),
        ..settings.clone()
    };
    let mut temp = path.as_os_str().to_owned();
    temp.push(".tmp");
    let temp = PathBuf::from(temp);
    fs::write(&temp, serialize(&clean))?;
    fs::rename(&temp, path).inspect_err(|_| {
        // The old file stays as it was; the temp file would only be left behind.
        let _ = fs::remove_file(&temp);
    })
}

#[cfg(test)]
mod tests;
