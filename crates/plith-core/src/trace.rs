//! Level history for the trace nodes: one ring per strip, 8 s at 30 samples a second, each
//! sample the loudest level since the previous one, normalised from dB to 0..1.

use std::collections::VecDeque;

/// Seconds of history a ring keeps.
pub const TRACE_SECS: f64 = 8.0;
/// Samples per second.
pub const TRACE_HZ: f64 = 30.0;
/// Samples a full ring holds (8 s at 30 Hz).
pub const TRACE_LEN: usize = 240;
/// The level at the bottom of a trace, in dB.
pub const FLOOR_DB: f32 = -60.0;

/// A linear peak (0 to 1, above on clipping) as a trace height: its dB value mapped from
/// [`FLOOR_DB`]..0 to 0..1. Silence, negative and NaN levels give 0; clipping gives 1.
#[must_use]
pub fn normalise(linear: f32) -> f32 {
    if linear.is_nan() || linear <= 0.0 {
        return 0.0;
    }
    let db = 20.0 * linear.log10();
    ((db - FLOOR_DB) / -FLOOR_DB).clamp(0.0, 1.0)
}

/// Slack for float drift when a frame lands exactly on the next sample time.
const EPSILON_S: f64 = 1e-6;

/// The recent history of one level, decimated by time.
#[derive(Debug, Clone, Default)]
pub struct TraceRing {
    samples: VecDeque<f32>,
    next_at: Option<f64>,
    pending: f32,
}

impl TraceRing {
    /// An empty ring.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Offers the linear level read at `now_s` seconds. At most one sample is kept per 1/30 s;
    /// a level offered in between raises the next sample. A gap longer than the window clears
    /// the ring first, so an old history never joins a new one.
    pub fn push(&mut self, now_s: f64, linear: f32) {
        let period = 1.0 / TRACE_HZ;
        let level = normalise(linear);
        if let Some(next_at) = self.next_at
            && now_s - next_at > TRACE_SECS
        {
            self.samples.clear();
            self.next_at = None;
            self.pending = 0.0;
        }
        self.pending = self.pending.max(level);
        let due = self
            .next_at
            .is_none_or(|next_at| now_s + EPSILON_S >= next_at);
        if !due {
            return;
        }
        if self.samples.len() == TRACE_LEN {
            self.samples.pop_front();
        }
        self.samples.push_back(self.pending);
        self.pending = 0.0;
        let next = self.next_at.unwrap_or(now_s) + period;
        // A late frame restarts the grid at its own time instead of catching up.
        self.next_at = Some(if next + EPSILON_S < now_s {
            now_s + period
        } else {
            next
        });
    }

    /// The kept samples, oldest first.
    #[must_use]
    pub fn samples(&self) -> Vec<f32> {
        self.samples.iter().copied().collect()
    }
}

#[cfg(test)]
mod tests;
