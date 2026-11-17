//! The supervisor's pure state machine: when to launch the helper, what counts as a failure,
//! when the link has stayed up long enough to forget earlier failures. No Windows calls.

use plith_core::osd::policy::{DOWN_GRACE, Next, Relaunch};
use std::time::{Duration, Instant};

/// How long the link must stay connected before the failure history is cleared. A helper that
/// connects and dies at once must still reach the give-up count.
pub const STABLE_UP: Duration = Duration::from_secs(10);

/// What the supervisor knows about the helper it started.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelperState {
    /// No helper of ours is being watched (none started, or the last one was let go).
    Absent,
    /// The helper we started is still running.
    Alive,
    /// The helper we started has ended.
    Exited,
}

/// Why a failure was counted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    /// The launch itself failed (helper missing, shell error).
    Launch,
    /// The helper we started ended.
    Exited,
    /// The link stayed down for [`DOWN_GRACE`].
    Down,
}

/// What one tick asks the supervisor to do and to log.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Step {
    /// Start the helper now, then report with [`Health::launched`].
    pub launch: bool,
    /// A failure counted in this tick; the supervisor lets its helper handle go.
    pub failure: Option<Failure>,
    /// This tick's failure made the supervisor give up.
    pub gave_up: bool,
    /// `Some(true)` when the link came up, `Some(false)` when it went down after being up.
    pub link: Option<bool>,
}

/// Where the launch cycle stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Launch at this instant (unless the link is already up then).
    Due(Instant),
    /// A helper was started (or one was already serving): watch it.
    Watching,
    /// Too many failures; never launch again this session.
    GaveUp,
}

/// The launch and failure bookkeeping of one session.
#[derive(Debug, Clone)]
pub struct Health {
    relaunch: Relaunch,
    phase: Phase,
    connected: bool,
    down_since: Option<Instant>,
    up_since: Option<Instant>,
    credited: bool,
    /// Our helper exited while the link still looked up: the link is not trusted until it
    /// goes down (the link notices a dead server only at its next write).
    stale: bool,
}

impl Health {
    /// A session that wants its first launch at `now`, with the link down.
    #[must_use]
    pub fn new(now: Instant) -> Self {
        Self {
            relaunch: Relaunch::new(),
            phase: Phase::Due(now),
            connected: false,
            down_since: Some(now),
            up_since: None,
            credited: false,
            stale: false,
        }
    }

    /// Whether the supervisor has given up launching.
    #[cfg(test)]
    #[must_use]
    pub fn gave_up(&self) -> bool {
        self.phase == Phase::GaveUp
    }

    /// Whether the link is up and trusted: connected, and our helper has not exited since it
    /// connected. Suppression follows this, so a dead helper never leaves the flyout hidden.
    #[must_use]
    pub fn link_up(&self) -> bool {
        self.connected && !self.stale
    }

    /// One look at the link and the helper at `now`.
    pub fn tick(&mut self, now: Instant, connected: bool, helper: HelperState) -> Step {
        let mut step = Step {
            link: self.track_link(now, connected),
            ..Step::default()
        };
        if self.phase == Phase::Watching {
            let down_long = self
                .down_since
                .is_some_and(|since| now.saturating_duration_since(since) >= DOWN_GRACE);
            let failure = if helper == HelperState::Exited {
                Some(Failure::Exited)
            } else if down_long {
                Some(Failure::Down)
            } else {
                None
            };
            if let Some(failure) = failure {
                self.stale |= failure == Failure::Exited && connected;
                step.failure = Some(failure);
                step.gave_up = self.fail(now);
            }
        }
        if let Phase::Due(at) = self.phase
            && now >= at
        {
            if self.link_up() {
                // A helper already serves the pipe (a slow one of ours, or one started by
                // hand): another launch would only exit and count as a failure. A stale link
                // (our helper died behind it) does not count: launch.
                self.phase = Phase::Watching;
            } else {
                step.launch = true;
            }
        }
        step
    }

    /// Reports the launch that the last [`Step::launch`] asked for. Returns whether this made
    /// the supervisor give up.
    pub fn launched(&mut self, now: Instant, ok: bool) -> bool {
        if ok {
            self.phase = Phase::Watching;
            if !self.connected {
                // The new helper gets the whole grace to connect.
                self.down_since = Some(now);
            }
            false
        } else {
            self.fail(now)
        }
    }

    /// Updates the link timers; returns the change, if any.
    fn track_link(&mut self, now: Instant, connected: bool) -> Option<bool> {
        let was = self.connected;
        self.connected = connected;
        if connected {
            self.down_since = None;
            let since = *self.up_since.get_or_insert(now);
            if !self.credited && !self.stale && now.saturating_duration_since(since) >= STABLE_UP {
                self.relaunch.on_connected();
                self.credited = true;
            }
        } else {
            self.up_since = None;
            self.credited = false;
            self.stale = false;
            self.down_since.get_or_insert(now);
        }
        (was != connected).then_some(connected)
    }

    /// Counts a failure at `now`; returns whether that made the supervisor give up.
    fn fail(&mut self, now: Instant) -> bool {
        match self.relaunch.on_failure(now) {
            Next::At(at) => {
                self.phase = Phase::Due(at);
                false
            }
            Next::GiveUp => {
                self.phase = Phase::GaveUp;
                true
            }
        }
    }
}

/// Remembers which failure texts were logged, so each is written once until the link comes up.
#[derive(Debug, Default)]
pub struct Notes {
    said: Vec<String>,
}

impl Notes {
    /// Whether `text` is new since the last [`clear`](Self::clear); records it.
    pub fn fresh(&mut self, text: &str) -> bool {
        if self.said.iter().any(|said| said == text) {
            return false;
        }
        self.said.push(text.to_string());
        true
    }

    /// Forgets every text (the link came up, so a later failure is news again).
    pub fn clear(&mut self) {
        self.said.clear();
    }
}

/// The log line for a counted failure; `detail` is the launch error or the link's last error.
#[must_use]
pub fn failure_line(failure: Failure, detail: Option<&str>) -> String {
    let detail = detail.unwrap_or("no error reported");
    match failure {
        Failure::Launch => format!("osd: helper launch failed: {detail}"),
        Failure::Exited => format!("osd: helper exited (link: {detail})"),
        Failure::Down => format!(
            "osd: link down for {} s (link: {detail})",
            DOWN_GRACE.as_secs()
        ),
    }
}

/// The line written once when the supervisor gives up.
#[must_use]
pub fn give_up_line() -> String {
    format!(
        "osd: gave up after {} failures within {} s; a helper started by hand still connects",
        plith_core::osd::policy::GIVE_UP_AFTER,
        plith_core::osd::policy::FAILURE_WINDOW.as_secs()
    )
}

#[cfg(test)]
mod tests;
