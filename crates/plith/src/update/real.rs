//! The real [`Backend`]: `WinHTTP`, the system clock, the shell's busy state and the installer
//! launch.

use super::{Backend, LaunchError, launch};
use plith_core::setup::Version;
use plith_host::http::{self, NOT_FOUND};
use plith_host::log::data_dir;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// The latest-release endpoint of the Plith repository.
const LATEST: &str = "https://api.github.com/repos/berkeerdo/Plith/releases/latest";
/// The GitHub API media type.
const ACCEPT: &str = "application/vnd.github+json";

/// Talks to GitHub and Windows.
pub struct RealBackend;

impl Backend for RealBackend {
    fn latest(&self) -> Result<Option<String>, String> {
        match http::get_with_accept(LATEST, Some(ACCEPT)) {
            Ok(text) => Ok(Some(text)),
            // No release published yet is a successful check with nothing new.
            Err(reason) if reason == NOT_FOUND => Ok(None),
            Err(reason) => Err(reason),
        }
    }

    fn download(&self, url: &str, dest: &Path, progress: &mut dyn FnMut(u8)) -> Result<(), String> {
        http::download(url, dest, progress)
    }

    fn launch(&self, setup: &Path, own: Version) -> Result<(), LaunchError> {
        launch::verify_and_start(setup, own)
    }

    fn busy(&self) -> bool {
        plith_shell::user_busy().is_some()
    }

    fn now(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs())
    }

    fn updates_dir(&self) -> Result<PathBuf, String> {
        data_dir()
            .map(|dir| dir.join("updates"))
            .map_err(|e| format!("no data folder: {e}"))
    }
}
