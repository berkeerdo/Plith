//! The update rules: reading the latest GitHub release, when to check next, and when to tell
//! the user. Pure; the request, the download and the signature check live outside this crate.

use crate::json;
use crate::setup::Version;

/// Where a release asset must be downloaded from; the download runs a program.
const DOWNLOAD_PREFIX: &str = "https://github.com/berkeerdo/Plith/releases/download/";
/// Where the release page must live.
const PAGE_PREFIX: &str = "https://github.com/berkeerdo/Plith/releases/";

/// Seconds to wait after start before the first check.
const FIRST_CHECK: u64 = 120;
/// Seconds between checks after a success.
const AFTER_SUCCESS: u64 = 86_400;
/// Seconds before a retry after a failure.
const AFTER_FAILURE: u64 = 21_600;

/// A published release that carries an installer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    /// The release version.
    pub version: Version,
    /// The installer download address.
    pub setup_url: String,
    /// The release page address.
    pub page_url: String,
}

/// What the About row shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateRow {
    /// Up to date.
    Current,
    /// A check is running.
    Checking,
    /// A newer version is ready to install.
    Ready(Version),
    /// The installer is downloading, percent done.
    Downloading(u8),
    /// The downloaded file failed the signature check and was deleted.
    BadSignature,
    /// The check or the download failed.
    Failed,
}

/// Reads the GitHub "latest release" JSON. `None` when it is malformed, has no
/// `Plith-Setup-<version>.exe` asset, or a link does not point at the Plith releases.
#[must_use]
pub fn parse_release(text: &str) -> Option<Release> {
    let root = json::parse(text).ok()?;
    let version = Version::parse(root.get("tag_name")?.as_str()?)?;
    let page_url = root.get("html_url")?.as_str()?;
    if !page_url.starts_with(PAGE_PREFIX) {
        return None;
    }
    let wanted = format!("Plith-Setup-{version}.exe");
    let setup_url = root
        .get("assets")?
        .as_arr()?
        .iter()
        .find(|asset| asset.get("name").and_then(json::Json::as_str) == Some(wanted.as_str()))?
        .get("browser_download_url")?
        .as_str()?;
    if !setup_url.starts_with(DOWNLOAD_PREFIX) {
        return None;
    }
    Some(Release {
        version,
        setup_url: setup_url.to_string(),
        page_url: page_url.to_string(),
    })
}

/// Seconds to wait from `now` for the next check: two minutes after start, a day after the last
/// success, six hours after a failure that came after it. Never negative; a clock that went
/// backwards (a recorded time in the future) means check now.
#[must_use]
pub fn next_check_in(now: u64, started: u64, last_ok: u64, last_failed: Option<u64>) -> u64 {
    if last_ok > now || last_failed.is_some_and(|failed| failed > now) {
        return 0;
    }
    let due = match last_failed {
        Some(failed) if failed >= last_ok => failed + AFTER_FAILURE,
        _ if last_ok > 0 => last_ok + AFTER_SUCCESS,
        _ => 0,
    };
    due.max(started.saturating_add(FIRST_CHECK))
        .saturating_sub(now)
}

/// Whether to tell the user about `release`: it is newer than `own` and not the version already
/// noticed (`seen`; empty or unparsable means none).
#[must_use]
pub fn should_notice(release: &Version, own: Version, seen: &str) -> bool {
    *release > own && Version::parse(seen) != Some(*release)
}

#[cfg(test)]
mod tests;
