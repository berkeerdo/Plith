//! Login state machine: tells a transient engine hiccup from a dead engine.

use std::time::{Duration, Instant};

const GRACE_RUNNING: Duration = Duration::from_secs(5);
const GRACE_COLD_START: Duration = Duration::from_secs(30);
const NEGATIVES_TO_GONE: u32 = 5;

/// Result of one `IsParametersDirty` poll.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirtyOutcome {
    /// Nothing changed.
    Clean,
    /// Parameters changed, re-read them.
    Dirty,
    /// Negative result that is not yet proof the engine is gone.
    Transient,
    /// Enough negatives past the grace period: treat the engine as gone.
    EngineGone,
}

/// Pure login state; the caller passes the clock.
#[derive(Debug, Clone, Copy)]
pub struct LoginSession {
    grace_until: Instant,
    negatives: u32,
}

impl LoginSession {
    /// Starts a session from the `VBVMR_Login` return code (0 running, 1 engine launched).
    #[must_use]
    pub fn on_login(rc: i32, now: Instant) -> Self {
        let grace = if rc == 0 {
            GRACE_RUNNING
        } else {
            GRACE_COLD_START
        };
        Self {
            grace_until: now + grace,
            negatives: 0,
        }
    }

    /// Classifies an `IsParametersDirty` return code. Never asks for a Logout.
    pub fn on_dirty(&mut self, rc: i32, now: Instant) -> DirtyOutcome {
        if rc >= 0 {
            self.negatives = 0;
            return if rc == 0 {
                DirtyOutcome::Clean
            } else {
                DirtyOutcome::Dirty
            };
        }
        if now < self.grace_until {
            return DirtyOutcome::Transient;
        }
        self.negatives += 1;
        if self.negatives >= NEGATIVES_TO_GONE {
            DirtyOutcome::EngineGone
        } else {
            DirtyOutcome::Transient
        }
    }
}
