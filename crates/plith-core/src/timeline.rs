//! Vsync timeline: snaps jittery vblank-wait wake-ups to a period grid that tracks the earliest
//! wake-up (a vblank wait only returns late), and stamps
//! animation time to the vblank a frame will be shown on.

/// Fraction of a late wake-up's lateness the grid follows per step (it tracks the earliest).
const LATE_GAIN: f64 = 0.005;
/// A wake-up later than this many periods resynchronises the grid.
const RESYNC_PERIODS: f64 = 30.0;
/// Fraction of a period a wake-up may be late and still belong to its vblank.
const LATE_ALLOWANCE: f64 = 0.25;
/// Lateness, as a fraction of a period, beyond which a step counts as off-rhythm.
const OFF_RHYTHM_LATENESS: f64 = 0.5;
/// Off-rhythm score above which the display rate should be read again.
const REREAD_SCORE: u32 = 20;

/// Converts a whole, non-negative period count to `u32`, saturating (a resync can span any
/// number of periods).
// Allowed: `n` is a whole, non-negative value; a float to int cast saturates.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn whole_periods(n: f64) -> u32 {
    n as u32
}

/// A locked vblank, in seconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tick {
    /// Time of the vblank on the phase-locked grid.
    pub vblank: f64,
    /// Vblank period (always nominal).
    pub period: f64,
    /// Whole periods since the previous tick: 1 on the first anchor, the rounded gap on a
    /// resync after a long stall. `dt` derived from it can therefore be large; springs clamp it.
    pub periods: u32,
    /// Seconds the raw wake-up came after the locked vblank (0 on a snap and on the first anchor).
    pub late: f64,
}

/// Phase-locked estimate of the display's vblank times.
#[derive(Debug, Clone)]
pub struct VsyncTimeline {
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
            // A long stall: re-anchor the grid, but report the whole gap so it shows as a hitch.
            let gap = ((raw - last) / self.period).round().max(1.0);
            let mut tick = self.anchor(raw);
            tick.periods = whole_periods(gap);
            return Some(tick);
        }
        let predicted = last + n * self.period;
        let error = raw - predicted;
        // Missed vblanks (n > 1) count too: a rate mismatch skips vblanks with modest lateness.
        if error > OFF_RHYTHM_LATENESS * self.period || n > 1.0 {
            self.off_rhythm += 1;
        } else {
            self.off_rhythm = self.off_rhythm.saturating_sub(1);
        }
        // A vblank wait only returns late: snap to an early wake-up, follow a late one slowly.
        let vblank = if error < 0.0 {
            raw
        } else {
            predicted + LATE_GAIN * error
        };
        self.last = Some(vblank);
        Some(Tick {
            vblank,
            period: self.period,
            periods: whole_periods(n),
            late: (raw - vblank).max(0.0),
        })
    }

    fn anchor(&mut self, raw: f64) -> Tick {
        self.last = Some(raw);
        Tick {
            vblank: raw,
            period: self.period,
            periods: 1,
            late: 0.0,
        }
    }

    /// Forgets the grid; the next `lock` anchors at its raw time.
    pub fn reset(&mut self) {
        self.last = None;
        self.off_rhythm = 0;
    }

    /// Switches to a new nominal refresh rate and resets the grid.
    pub fn set_nominal(&mut self, hz: f64) {
        self.period = 1.0 / hz;
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
    /// Returns `(anim_t, dt)`. The first stamp after default or reset anchors at the frame's
    /// shown time (one period after its vblank) with `dt` of one period. Later steps are
    /// exactly `periods * period`, never a vblank difference, so grid snaps cannot reach physics.
    pub fn stamp(&mut self, tick: Tick) -> (f64, f64) {
        let (anim_t, dt) = match self.last_anim {
            None => (tick.vblank + tick.period, tick.period),
            Some(last) => {
                let dt = f64::from(tick.periods) * tick.period;
                (last + dt, dt)
            }
        };
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
