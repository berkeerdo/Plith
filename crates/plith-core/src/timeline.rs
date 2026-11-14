//! Vsync timeline: snaps jittery vblank-wait wake-ups to a phase-locked period grid, and stamps
//! animation time to the vblank a frame will be shown on.

/// Phase correction applied to the grid per locked step.
const PHASE_GAIN: f64 = 0.1;
/// Period correction applied per locked step.
const PERIOD_GAIN: f64 = 0.02;
/// Allowed deviation of the period from nominal, as a fraction.
const PERIOD_TOLERANCE: f64 = 0.03;
/// A wake-up later than this many periods resynchronises the grid.
const RESYNC_PERIODS: f64 = 30.0;
/// Fraction of a period a wake-up may be late and still belong to its vblank.
const LATE_ALLOWANCE: f64 = 0.25;
/// Phase error, as a fraction of a period, beyond which a step counts as off-rhythm.
const OFF_RHYTHM_ERROR: f64 = 0.25;
/// Off-rhythm score above which the display rate should be read again.
const REREAD_SCORE: u32 = 20;

/// Converts a whole, non-negative period count (here always 1..=30) to `u32`.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn whole_periods(n: f64) -> u32 {
    n as u32
}

/// A locked vblank, in seconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tick {
    /// Time of the vblank on the phase-locked grid.
    pub vblank: f64,
    /// Current estimate of the vblank period.
    pub period: f64,
    /// Whole periods since the previous tick (1 on anchor and resync).
    pub periods: u32,
}

/// Phase-locked estimate of the display's vblank times.
#[derive(Debug, Clone)]
pub struct VsyncTimeline {
    nominal: f64,
    period: f64,
    last: Option<f64>,
    off_rhythm: u32,
}

impl VsyncTimeline {
    /// Creates a timeline for a display refreshing at `nominal_hz`.
    #[must_use]
    pub fn new(nominal_hz: f64) -> Self {
        let period = 1.0 / nominal_hz;
        Self {
            nominal: period,
            period,
            last: None,
            off_rhythm: 0,
        }
    }

    /// Feeds a raw wake-up time. Returns `None` for a spurious wake-up (no new vblank).
    pub fn lock(&mut self, raw: f64) -> Option<Tick> {
        let Some(last) = self.last else {
            return Some(self.anchor(raw));
        };
        let n = ((raw - last) / self.period + LATE_ALLOWANCE).floor();
        if n < 1.0 {
            return None;
        }
        if n > RESYNC_PERIODS {
            self.period = self.nominal;
            return Some(self.anchor(raw));
        }
        let predicted = last + n * self.period;
        let error = raw - predicted;
        if error.abs() > OFF_RHYTHM_ERROR * self.period {
            self.off_rhythm += 1;
        } else {
            self.off_rhythm = self.off_rhythm.saturating_sub(1);
        }
        let vblank = predicted + PHASE_GAIN * error;
        let low = self.nominal * (1.0 - PERIOD_TOLERANCE);
        let high = self.nominal * (1.0 + PERIOD_TOLERANCE);
        self.period = (self.period + PERIOD_GAIN * error / n).clamp(low, high);
        self.last = Some(vblank);
        Some(Tick {
            vblank,
            period: self.period,
            periods: whole_periods(n),
        })
    }

    fn anchor(&mut self, raw: f64) -> Tick {
        self.last = Some(raw);
        Tick {
            vblank: raw,
            period: self.period,
            periods: 1,
        }
    }

    /// Forgets the grid; the next `lock` anchors at its raw time.
    pub fn reset(&mut self) {
        self.last = None;
        self.off_rhythm = 0;
    }

    /// Switches to a new nominal refresh rate and resets the grid.
    pub fn set_nominal(&mut self, hz: f64) {
        self.nominal = 1.0 / hz;
        self.period = self.nominal;
        self.reset();
    }

    /// True while wake-ups keep missing the grid, so the display rate may have changed.
    #[must_use]
    pub fn needs_rate_reread(&self) -> bool {
        self.off_rhythm > REREAD_SCORE
    }
}

/// Turns ticks into animation time stamps and frame steps.
#[derive(Debug, Clone, Copy, Default)]
pub struct FrameStamper {
    last_anim: Option<f64>,
}

impl FrameStamper {
    /// Returns `(anim_t, dt)`: the frame is shown one period after the vblank it was drawn
    /// for. `dt` is one period for the first stamp after default or reset.
    pub fn stamp(&mut self, tick: Tick) -> (f64, f64) {
        let anim_t = tick.vblank + tick.period;
        let dt = self.last_anim.map_or(tick.period, |last| anim_t - last);
        self.last_anim = Some(anim_t);
        (anim_t, dt)
    }

    /// Forgets the previous stamp.
    pub fn reset(&mut self) {
        self.last_anim = None;
    }
}

#[cfg(test)]
mod tests;
