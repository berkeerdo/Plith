//! Suppression of the native Windows volume flyout, so Plith's OSD is the only one on screen.
//!
//! Pure layer (this file and [`registry`]): [`should_hide`] decides from [`WindowFacts`],
//! [`SuppressionWindow`] is the 400 ms window a volume event opens, [`HiddenRegistry`] remembers
//! what was hidden. Windows layer: `hook` (`WinEvent` hooks on their own message thread).

pub mod registry;

#[cfg(windows)]
mod hook;
#[cfg(test)]
mod tests;

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

pub use registry::HiddenRegistry;

#[cfg(windows)]
pub use hook::FlyoutSuppressor;

/// How long after a volume event a matching window may still be hidden.
pub const WINDOW_DURATION: Duration = Duration::from_millis(400);

/// Window classes the shell uses to host the volume flyout across Windows builds.
const HOST_CLASSES: [&str; 5] = [
    "NativeHWNDHost",
    "Xaml_WindowedPopupClass",
    "Microsoft.UI.Content.DesktopChildSiteBridge",
    "Windows.UI.Composition.DesktopWindowContentBridge",
    "XamlExplorerHostIslandWindow",
];

/// Whether a window class name, as UTF-16 straight from `GetClassNameW`, is one of
/// [`HOST_CLASSES`]. Compares in place: no allocation, so the event callback can run it first.
#[must_use]
pub fn is_host_class(class: &[u16]) -> bool {
    HOST_CLASSES
        .iter()
        .any(|name| class.iter().copied().eq(name.encode_utf16()))
}

/// Owning process names (no extension) the flyout has been seen under.
const SHELL_PROCESSES: [&str; 3] = ["ShellExperienceHost", "Explorer", "WindowsInputExperience"];

/// Z-bands the shell has used for the flyout (notifications, then newer shell bands).
const FLYOUT_BANDS: [u32; 4] = [0x4, 0x10, 0x11, 0x12];

/// What the suppressor does with a window that passes every filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Only report the window; nothing is hidden. Used by probes.
    DryRun,
    /// Hide the window with `SW_HIDE` and restore it on stop.
    Hide,
}

/// What is known about a window an event named.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowFacts {
    /// Window class name.
    pub class: String,
    /// Owning process name without extension.
    pub process: String,
    /// Z-band, or `None` when it could not be read (the export is undocumented).
    pub band: Option<u32>,
}

/// A window that passed every filter, reported to the owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    /// The window handle value.
    pub hwnd: isize,
    /// What identified it.
    pub facts: WindowFacts,
    /// Whether it was actually hidden (always `false` in [`Mode::DryRun`]).
    pub hidden: bool,
}

/// True when `facts` names the shell's volume flyout and a volume event is recent.
///
/// Class and process must match; an unknown band is allowed so older Windows without
/// `GetWindowBand` still gets the class and process filtering.
#[must_use]
pub fn should_hide(facts: &WindowFacts, window_open: bool) -> bool {
    window_open
        && HOST_CLASSES.contains(&facts.class.as_str())
        && SHELL_PROCESSES
            .iter()
            .any(|name| name.eq_ignore_ascii_case(&facts.process))
        && facts.band.is_none_or(|band| FLYOUT_BANDS.contains(&band))
}

/// The window a volume event opens. Lock free, so the event callback and the thread that
/// opens it never wait on each other; the clock is passed in so tests control time.
#[derive(Debug)]
pub struct SuppressionWindow {
    base: Instant,
    /// Milliseconds after `base` at which the window closes, plus one (0 means never opened).
    closes_at: AtomicU64,
}

impl SuppressionWindow {
    /// A closed window; `now` is the reference all later instants are measured from.
    #[must_use]
    pub fn new(now: Instant) -> Self {
        Self {
            base: now,
            closes_at: AtomicU64::new(0),
        }
    }

    /// Opens (or extends) the window from `now`.
    pub fn open(&self, now: Instant) {
        let end = now.saturating_duration_since(self.base) + WINDOW_DURATION;
        let millis = u64::try_from(end.as_millis()).unwrap_or(u64::MAX - 1);
        self.closes_at.store(millis + 1, Ordering::Relaxed);
    }

    /// Whether `now` is inside the window (the closing instant itself is still open).
    #[must_use]
    pub fn is_open(&self, now: Instant) -> bool {
        let closes = self.closes_at.load(Ordering::Relaxed);
        if closes == 0 {
            return false;
        }
        let elapsed = now.saturating_duration_since(self.base).as_millis();
        elapsed < u128::from(closes)
    }
}
