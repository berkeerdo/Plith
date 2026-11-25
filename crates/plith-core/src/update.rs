//! The update rules: reading the latest GitHub release, when to check next, and when to tell
//! the user. Pure; the request, the download and the signature check live outside this crate.

use crate::json;
use crate::setup::Version;

/// The Plith releases address; every accepted link is built from it, never matched by prefix.
const RELEASES: &str = "https://github.com/berkeerdo/Plith/releases";

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
    /// Not checked since Plith started: no claim, only the check button.
    Unknown,
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
/// `Plith-Setup-<version>.exe` asset, or a link is not exactly the address built from the tag:
/// `.../releases/download/<tag>/Plith-Setup-<version>.exe` and `.../releases/tag/<tag>`. The tag
/// may hold only letters, digits, `.` and `-`, with no `..`.
#[must_use]
pub fn parse_release(text: &str) -> Option<Release> {
    let root = json::parse(text).ok()?;
    let tag = root.get("tag_name")?.as_str()?;
    let safe = !tag.is_empty()
        && !tag.contains("..")
        && tag
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-');
    if !safe {
        return None;
    }
    let version = Version::parse(tag)?;
    let name = format!("Plith-Setup-{version}.exe");
    let page_url = format!("{RELEASES}/tag/{tag}");
    let setup_url = format!("{RELEASES}/download/{tag}/{name}");
    if root.get("html_url")?.as_str()? != page_url {
        return None;
    }
    let asset = root
        .get("assets")?
        .as_arr()?
        .iter()
        .find(|asset| asset.get("name").and_then(json::Json::as_str) == Some(name.as_str()))?;
    if asset.get("browser_download_url")?.as_str()? != setup_url {
        return None;
    }
    Some(Release {
        version,
        setup_url,
        page_url,
    })
}

/// Seconds to wait from `now` for the next check: two minutes after start, a day after the last
/// success, six hours after a failure that came after it. Never negative. A record later than
/// `now` (the clock went backwards) counts as absent, so the two-minute start floor still applies.
#[must_use]
pub fn next_check_in(now: u64, started: u64, last_ok: u64, last_failed: Option<u64>) -> u64 {
    let last_ok = if last_ok > now { 0 } else { last_ok };
    let last_failed = last_failed.filter(|&failed| failed <= now);
    let due = match last_failed {
        Some(failed) if failed >= last_ok => failed.saturating_add(AFTER_FAILURE),
        _ if last_ok > 0 => last_ok.saturating_add(AFTER_SUCCESS),
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
