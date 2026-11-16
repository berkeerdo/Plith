//! The debounced per-strip signal flag: a strip carries signal while a peak above
//! [`SIGNAL_PEAK`] was seen within [`SIGNAL_HOLD`]. The poll thread publishes it in the
//! snapshot, so consumers wake once per edge, never at the level poll rate.

use std::time::{Duration, Instant};

/// Peak level above which a strip counts as carrying signal.
pub const SIGNAL_PEAK: f32 = 0.01;
/// How long signal is held after the last loud reading.
pub const SIGNAL_HOLD: Duration = Duration::from_secs(2);

/// Per-strip signal memory with the hold; reports the flags only when they change.
#[derive(Debug, Clone, Default)]
pub struct SignalHold {
    /// When each strip was last loud.
    loud: Vec<Option<Instant>>,
    /// The flags last reported.
    flags: Vec<bool>,
}

impl SignalHold {
    /// Records one reading of per-strip `[left, right]` peaks taken at `now`. Returns the new
    /// flags when they differ from the ones last reported (an edge), else `None`.
    pub fn update(&mut self, strips: &[[f32; 2]], now: Instant) -> Option<Vec<bool>> {
        self.loud.resize(strips.len(), None);
        for (slot, peaks) in self.loud.iter_mut().zip(strips) {
            if peaks[0].max(peaks[1]) > SIGNAL_PEAK {
                *slot = Some(now);
            }
        }
        let flags: Vec<bool> = self
            .loud
            .iter()
            .map(|t| t.is_some_and(|t| now.saturating_duration_since(t) <= SIGNAL_HOLD))
            .collect();
        if flags == self.flags {
            return None;
        }
        self.flags.clone_from(&flags);
        Some(flags)
    }

    /// The flags last reported.
    #[must_use]
    pub fn flags(&self) -> &[bool] {
        &self.flags
    }

    /// The flags last reported, cut or padded with `false` to `strips` entries.
    #[must_use]
    pub fn flags_for(&self, strips: usize) -> Vec<bool> {
        let mut flags = self.flags.clone();
        flags.resize(strips, false);
        flags
    }

    /// Forgets everything, as when the engine goes away.
    pub fn reset(&mut self) {
        self.loud.clear();
        self.flags.clear();
    }
}
