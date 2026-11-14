//! Which discovery run is the current one. Pure.
//!
//! A `start` that timed out leaves its worker running until the driver call returns. That
//! zombie must not overwrite what a newer run published, so every run carries a token and only
//! the newest token may publish.

use std::sync::atomic::{AtomicU64, Ordering};

/// Hands out run tokens; the last one handed out is the only current one.
#[derive(Debug, Default)]
pub struct RunTokens(AtomicU64);

impl RunTokens {
    /// Starts a new run: every earlier token stops being current.
    pub fn begin(&self) -> u64 {
        self.0.fetch_add(1, Ordering::AcqRel) + 1
    }

    /// Whether `token` belongs to the newest run.
    #[must_use]
    pub fn is_current(&self, token: u64) -> bool {
        self.0.load(Ordering::Acquire) == token
    }

    /// The newest token.
    #[must_use]
    pub fn latest(&self) -> u64 {
        self.0.load(Ordering::Acquire)
    }
}
