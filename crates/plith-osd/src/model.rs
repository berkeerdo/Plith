//! The OSD state: what shows, how far each spring has come, when to hide and when to exit.
//!
//! Pure: no Windows calls, every time is an explicit `now` in seconds on one time base (the
//! render thread uses seconds since its origin, the same base the compositor clock reports).

use plith_core::spring::Spring;
use plith_core::tuning::{OPACITY, Tuning};
use plith_ipc::{Message, Meter};

/// How long the OSD stays after the last `Level`, in seconds.
pub const HOLD_S: f64 = 1.5;
/// How long the helper waits for a new client after the last one went away, in seconds.
pub const EXIT_GRACE_S: f64 = 2.0;
/// The ruler fill spring, in percent: critically damped (zeta 1), so it never overshoots.
pub const LEVEL: Tuning = Tuning::new(400.0, 40.0);

/// What the painter draws for one frame.
#[derive(Debug, Clone, PartialEq)]
pub struct OsdFrame {
    /// Whole-OSD opacity, 0 to 1.
    pub opacity: f32,
    /// Ruler fill, 0 to 1.
    pub level: f32,
    /// What the ruler measures.
    pub meter: Meter,
    /// Whether the output is muted.
    pub muted: bool,
    /// The device short name, as sent.
    pub label: String,
}

/// The OSD state machine.
#[derive(Debug)]
pub struct OsdModel {
    opacity: Spring,
    /// Ruler fill in percent, 0 to 100.
    level: Spring,
    meter: Meter,
    muted: bool,
    label: String,
    hide_at: Option<f64>,
    exit_at: Option<f64>,
    bye: bool,
    /// Time of the previous step while animating; `None` at rest.
    last: Option<f64>,
    /// The frame changed without a spring moving (mute, label, a snapped level).
    dirty: bool,
}

impl Default for OsdModel {
    fn default() -> Self {
        Self::new()
    }
}

/// Narrows a value already clamped to 0..=1.
// Allowed: the value is clamped to 0..=1, well inside f32 range and precision.
#[allow(clippy::cast_possible_truncation)]
fn unit_f32(value: f64) -> f32 {
    value.clamp(0.0, 1.0) as f32
}

impl OsdModel {
    /// A hidden OSD with no client.
    #[must_use]
    pub fn new() -> Self {
        Self {
            opacity: Spring::new(0.0, OPACITY).with_eps(0.002, 0.01),
            level: Spring::new(0.0, LEVEL),
            meter: Meter::Volume,
            muted: false,
            label: String::new(),
            hide_at: None,
            exit_at: None,
            bye: false,
            last: None,
            dirty: false,
        }
    }

    /// Applies one message received at `now`. `Level` shows (or keeps showing) and arms the
    /// hide at `now + HOLD_S`; `Hide` fades out at once; `Hello` disarms a pending exit;
    /// `Bye` makes [`should_exit`](Self::should_exit) true.
    pub fn apply(&mut self, msg: &Message, now: f64) {
        match msg {
            Message::Hello { .. } => self.exit_at = None,
            Message::Level {
                meter,
                percent,
                muted,
                label,
            } => self.show(*meter, f64::from(*percent), *muted, label, now),
            Message::Hide => {
                self.hide_at = None;
                self.aim_opacity(0.0, now);
            }
            Message::Bye => self.bye = true,
        }
    }

    fn show(&mut self, meter: Meter, percent: f64, muted: bool, label: &str, now: f64) {
        // A show from hidden, or a different meter, starts at the new level: no sweep.
        let hidden = self.opacity.target() == 0.0 && self.opacity.value() == 0.0;
        if hidden || meter != self.meter {
            self.level = Spring::new(percent, LEVEL);
        } else {
            self.mark_moving(now);
            self.level.retarget(percent, LEVEL);
        }
        self.meter = meter;
        self.muted = muted;
        label.clone_into(&mut self.label);
        self.hide_at = Some(now + HOLD_S);
        self.dirty = true;
        self.aim_opacity(1.0, now);
    }

    fn aim_opacity(&mut self, target: f64, now: f64) {
        if (self.opacity.target() - target).abs() > f64::EPSILON {
            self.mark_moving(now);
            self.opacity.retarget(target, OPACITY);
        }
    }

    /// Starts the step clock at `now` when the model was at rest.
    fn mark_moving(&mut self, now: f64) {
        self.last.get_or_insert(now);
    }

    /// The client went away at `now` (its process ended): exit unless another `Hello`
    /// arrives within `EXIT_GRACE_S`.
    pub fn client_gone(&mut self, now: f64) {
        self.exit_at.get_or_insert(now + EXIT_GRACE_S);
    }

    /// Fires the hide timer when due and advances the springs to `now`. Returns true while
    /// animating, and once after a change that moved no spring (it needs one repaint); false
    /// at rest, where nothing changes until a message or the [`deadline`](Self::deadline).
    pub fn step(&mut self, now: f64) -> bool {
        if self.hide_at.is_some_and(|at| now >= at) {
            self.hide_at = None;
            self.aim_opacity(0.0, now);
        }
        let dt = self.last.map_or(0.0, |last| now - last);
        let opacity_settled = self.opacity.step(dt);
        let level_settled = self.level.step(dt);
        let animating = !(opacity_settled && level_settled);
        self.last = animating.then_some(now);
        let dirty = std::mem::take(&mut self.dirty);
        animating || dirty
    }

    /// The frame to draw.
    #[must_use]
    pub fn frame(&self) -> OsdFrame {
        OsdFrame {
            opacity: unit_f32(self.opacity.value()),
            level: unit_f32(self.level.value() / 100.0),
            meter: self.meter,
            muted: self.muted,
            label: self.label.clone(),
        }
    }

    /// Whether the helper should exit at `now`: after `Bye`, or `EXIT_GRACE_S` after the
    /// client went away with no new `Hello`.
    #[must_use]
    pub fn should_exit(&self, now: f64) -> bool {
        self.bye || self.exit_at.is_some_and(|at| now >= at)
    }

    /// The next time something is due while at rest (the hide or the exit), `None` when
    /// nothing is.
    #[must_use]
    pub fn deadline(&self) -> Option<f64> {
        match (self.hide_at, self.exit_at) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    /// Whether the OSD is shown or fading in (false while fading out or hidden).
    #[must_use]
    pub fn is_showing(&self) -> bool {
        self.opacity.target() > 0.0
    }

    /// Whether nothing is on screen and nothing is fading.
    #[must_use]
    pub fn is_hidden(&self) -> bool {
        self.opacity.target() == 0.0 && self.opacity.value() == 0.0
    }
}

#[cfg(test)]
mod tests;
