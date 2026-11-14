//! A generation counter for coalescing bursts of change events.
//!
//! Event callbacks call [`Generation::bump`]; a reader notes [`Generation::current`] before a
//! slow read and publishes the result only while [`Generation::is_current`] still holds, so a
//! stale read never overwrites a newer one.

use std::sync::atomic::{AtomicU64, Ordering};

/// Shared, monotonically increasing change counter.
#[derive(Debug, Default)]
pub struct Generation(AtomicU64);

impl Generation {
    /// Records a change and returns the new generation.
    pub fn bump(&self) -> u64 {
        self.0.fetch_add(1, Ordering::AcqRel) + 1
    }

    /// The latest generation.
    #[must_use]
    pub fn current(&self) -> u64 {
        self.0.load(Ordering::Acquire)
    }

    /// Whether nothing changed since `seen` was read.
    #[must_use]
    pub fn is_current(&self, seen: u64) -> bool {
        self.current() == seen
    }
}

/// Whether a queued request outlived its caller's deadline and must be dropped, not run late.
pub(crate) fn expired(deadline: std::time::Instant, now: std::time::Instant) -> bool {
    now > deadline
}
