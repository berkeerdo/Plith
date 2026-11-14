//! The topmost fight: another window may take the top of the z-order after a foreground change,
//! so the island raises itself at once and again a few times, then stops.

/// Retries after the immediate raise.
pub const RETRIES: u32 = 4;
/// Delay between retries in milliseconds.
pub const RETRY_MS: u32 = 500;

/// What the shell should do next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopmostAction {
    /// Raise the window to the top, then arm a timer if `retry_in_ms` is set.
    Raise {
        /// Delay before the next timer tick, or `None` when the fight is over.
        retry_in_ms: Option<u32>,
    },
    /// Nothing to do.
    Idle,
}

/// Counts the retries left after the last foreground change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TopmostFight {
    remaining: u32,
}

impl TopmostFight {
    /// A foreground change: raise now and restart the retry count.
    pub fn on_foreground(&mut self) -> TopmostAction {
        self.remaining = RETRIES;
        TopmostAction::Raise {
            retry_in_ms: Some(RETRY_MS),
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
        }
    }
}

#[cfg(test)]
mod tests;
