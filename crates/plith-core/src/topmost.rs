//! The topmost fight: another window may take the top of the z-order after a foreground change,
//! so the island raises itself at once and again a few times, then stops. An alt-tab produces
//! several foreground events within a few milliseconds; only the first raises at once.

/// Retries after the immediate raise.
pub const RETRIES: u32 = 4;
/// Delay between retries in milliseconds.
pub const RETRY_MS: u32 = 500;
/// A foreground change this soon after the last raise only re-arms the retry timer.
pub const DEBOUNCE_MS: u64 = 100;

/// What the shell should do next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopmostAction {
    /// Arm a timer if `retry_in_ms` is set, and raise the window to the top when `immediate`.
    Raise {
        /// Delay before the next timer tick, or `None` when the fight is over.
        retry_in_ms: Option<u32>,
        /// Raise now. `false` for a debounced foreground change: only re-arm the timer.
        immediate: bool,
    },
    /// Nothing to do.
    Idle,
}

/// Counts the retries left after the last foreground change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TopmostFight {
    remaining: u32,
    last_raise_ms: Option<u64>,
}

impl TopmostFight {
    /// A foreground change at `now_ms` (a monotonic clock): restart the retry count and raise
    /// now, unless the last raise was less than [`DEBOUNCE_MS`] ago.
    pub fn on_foreground(&mut self, now_ms: u64) -> TopmostAction {
        self.remaining = RETRIES;
        let immediate = self
            .last_raise_ms
            .is_none_or(|last| now_ms.saturating_sub(last) >= DEBOUNCE_MS);
        if immediate {
            self.last_raise_ms = Some(now_ms);
        }
        TopmostAction::Raise {
            retry_in_ms: Some(RETRY_MS),
            immediate,
        }
    }

    /// The retry timer fired: raise again while retries are left.
    pub fn on_timer(&mut self) -> TopmostAction {
        if self.remaining == 0 {
            return TopmostAction::Idle;
        }
        self.remaining -= 1;
        TopmostAction::Raise {
            retry_in_ms: (self.remaining > 0).then_some(RETRY_MS),
            immediate: true,
        }
    }
}

#[cfg(test)]
mod tests;
