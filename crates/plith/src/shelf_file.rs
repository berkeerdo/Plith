//! The shelf file `%LOCALAPPDATA%\Plith\shelf.txt`: one rooted path per line, newest first, the
//! WPF format. The text form lives in `plith-core`; this file reads and writes it.

use plith_core::shelf::{Probe, Shelf};
use plith_host::log::{self, LogFile};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The shelf file next to the log.
///
/// # Errors
/// Returns the error of the known-folder call.
pub fn default_path() -> io::Result<PathBuf> {
    Ok(log::data_dir()?.join("shelf.txt"))
}

/// What became of `path`. Only a definite `NotFound` counts as gone: an offline share or a
/// path without access stays on the shelf, so a save never erases a line over a sleeping NAS.
fn probe(path: &Path) -> Probe {
    match fs::metadata(path) {
        Ok(meta) if meta.is_dir() => Probe::Dir,
        Ok(_) => Probe::File,
        Err(error) if error.kind() == io::ErrorKind::NotFound => Probe::Gone,
        Err(_) => Probe::Unknown,
    }
}

/// Reads the saved shelf on its own thread, so an offline share cannot hold up the UI thread,
/// and calls `wake` when the result is ready to be taken from the returned channel.
#[must_use]
pub fn load_async(
    log: &std::sync::Arc<LogFile>,
    wake: impl Fn() + Send + 'static,
) -> std::sync::mpsc::Receiver<Shelf> {
    let (tx, rx) = std::sync::mpsc::channel();
    let thread_log = std::sync::Arc::clone(log);
    let spawned = std::thread::Builder::new()
        .name("shelf-load".to_owned())
        .spawn(move || {
            let _ = tx.send(load(&thread_log));
            wake();
        });
    if let Err(error) = spawned {
        // The sender went down with the closure: the channel is closed and the shelf starts
        // empty but never saves (the load never completes), so the file is left as it is.
        log.line(&format!("shelf: cannot start the load thread: {error}"));
    }
    rx
}

/// The shelf in the file at `path`: empty when there is none or it cannot be read, and without
/// the lines whose file or folder no longer exists.
#[must_use]
pub fn load_at(path: &Path) -> Shelf {
    fs::read_to_string(path).map_or_else(|_| Shelf::default(), |text| Shelf::parse(&text, probe))
}

/// Writes the shelf to `path`, creating the folder. The text goes to `shelf.txt.tmp` first and
/// is renamed over the old file, so a crash mid-write leaves the old shelf, never a cut one.
///
/// # Errors
/// Returns the I/O error when the folder or the file cannot be written.
pub fn save_at(path: &Path, shelf: &Shelf) -> io::Result<()> {
    if let Some(folder) = path.parent() {
        fs::create_dir_all(folder)?;
    }
    let mut temp = path.as_os_str().to_owned();
    temp.push(".tmp");
    let temp = PathBuf::from(temp);
    fs::write(&temp, shelf.serialize())?;
    fs::rename(&temp, path)
}

/// The saved shelf, or an empty one.
#[must_use]
pub fn load(log: &LogFile) -> Shelf {
    match default_path() {
        Ok(path) => load_at(&path),
        Err(error) => {
            log.line(&format!("shelf: no folder: {error}"));
            Shelf::default()
        }
    }
}

/// A saver that writes to the default file and logs a failure.
#[must_use]
pub fn saver(log: std::sync::Arc<LogFile>) -> Box<dyn Fn(&Shelf)> {
    Box::new(move |shelf| {
        let saved = default_path().and_then(|path| save_at(&path, shelf));
        if let Err(error) = saved {
            log.line(&format!("shelf: cannot save: {error}"));
        }
    })
}

#[cfg(test)]
mod tests;
