//! The relaunch and flyout-suppression policy for the OSD link.

use crate::launch::FlyoutMode;
use std::time::{Duration, Instant};

/// The wait before the next attempt to start the OSD process.
pub const LAUNCH_SPACING: Duration = Duration::from_secs(3);
/// This many failures inside [`FAILURE_WINDOW`] make the supervisor give up.
pub const GIVE_UP_AFTER: usize = 5;
/// How far back failures count.
pub const FAILURE_WINDOW: Duration = Duration::from_secs(60);
/// How long the link may stay down before the flyout is released.
pub const DOWN_GRACE: Duration = Duration::from_secs(3);

/// What to do after a failed launch or a lost link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Next {
    /// Try again at this instant.
    At(Instant),
    /// Stop trying.
    GiveUp,
}

/// Tracks recent failures to space out relaunches and eventually give up.
#[derive(Debug, Clone, Default)]
pub struct Relaunch {
    failures: Vec<Instant>,
}

impl Relaunch {
    /// A supervisor with no failures on record.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a failure at `now` and says what to do next.
    pub fn on_failure(&mut self, now: Instant) -> Next {
        self.failures
            .retain(|&at| now.saturating_duration_since(at) < FAILURE_WINDOW);
        self.failures.push(now);
        if self.failures.len() >= GIVE_UP_AFTER {
            Next::GiveUp
        } else {
            Next::At(now + LAUNCH_SPACING)
        }
    }

    /// The link is up: the failure history no longer applies.
    pub fn on_connected(&mut self) {
        self.failures.clear();
    }
}

/// How the Windows flyout should be suppressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuppressMode {
    /// Find the flyout and log, without hiding it.
    DryRun,
    /// Hide the flyout.
    Hide,
}

/// The suppression wanted for `mode` while the OSD link is `link_up`.
///
/// Hiding the flyout without a working OSD would leave the user with no feedback at all.
#[must_use]
pub fn wants_suppression(mode: FlyoutMode, link_up: bool) -> Option<SuppressMode> {
    match mode {
        FlyoutMode::Off => None,
        FlyoutMode::DryRun => Some(SuppressMode::DryRun),
        FlyoutMode::Hide => link_up.then_some(SuppressMode::Hide),
    }
}

#[cfg(test)]
mod tests;
