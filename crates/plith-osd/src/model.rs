//! The OSD state: what shows, how far each spring has come, when to hide and when to exit.
//!
//! Pure: no Windows calls, every time is an explicit `now` in seconds on one time base (the
//! render thread uses seconds since its origin, the same base the compositor clock reports).

use plith_core::lang::{Lang, Text, text};
use plith_core::spring::Spring;
use plith_core::tuning::{OPACITY, Tuning};
use plith_ipc::{Message, Meter};

/// How long the OSD stays after the last `Level`, in seconds.
pub const HOLD_S: f64 = 1.5;
/// How long the helper waits for a new client after the last one went away, in seconds.
pub const EXIT_GRACE_S: f64 = 2.0;
/// How long a new helper waits for its first `Hello`, in seconds from time zero.
pub const START_GRACE_S: f64 = 10.0;
/// The longest a fade-out after `Bye` may delay the exit, in seconds.
pub const BYE_FADE_S: f64 = 0.5;
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
    /// The word drawn instead of the percent while muted, as sent.
    pub muted_word: String,
}

/// The label drawn before any `Level` has set one.
const DEFAULT_LABEL: &str = "VOLUME";

/// Two hidden frames that touch every text slot (the label, the percent, the muted word), so a
/// prewarm pays the first-draw costs (text formats, layout cache, glyphs) before the first show.
#[must_use]
pub fn warm_frames(label: &str, muted_word: &str) -> [OsdFrame; 2] {
    let frame = |muted: bool| OsdFrame {
        opacity: 0.0,
        level: 1.0,
        meter: Meter::Volume,
        muted,
        label: label.to_owned(),
        muted_word: muted_word.to_owned(),
    };
    [frame(false), frame(true)]
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
    muted_word: String,
    hide_at: Option<f64>,
    exit_at: Option<f64>,
    /// When `Bye` arrived.
    bye: Option<f64>,
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
    /// A hidden OSD with no client. Time zero is the helper's start: without a `Hello` by
    /// `START_GRACE_S` it exits.
    #[must_use]
    pub fn new() -> Self {
        Self {
            opacity: Spring::new(0.0, OPACITY).with_eps(0.002, 0.01),
            level: Spring::new(0.0, LEVEL),
            meter: Meter::Volume,
            muted: false,
            label: String::new(),
            muted_word: String::new(),
            hide_at: None,
            exit_at: Some(START_GRACE_S),
            bye: None,
            last: None,
            dirty: false,
        }
    }

    /// Applies one message received at `now`. `Level` shows (or keeps showing) and arms the
    /// hide at `now + HOLD_S`; `Hide` fades out at once; `Hello` disarms a pending exit;
    /// `Bye` fades out and makes [`should_exit`](Self::should_exit) true once hidden (at most
    /// `BYE_FADE_S` later). Every message after `Bye` is ignored.
    pub fn apply(&mut self, msg: &Message, now: f64) {
        if self.bye.is_some() {
            return;
        }
        match msg {
            Message::Hello { .. } => self.exit_at = None,
            Message::Level {
                meter,
                percent,
                muted,
                label,
                muted_word,
            } => {
                muted_word.clone_into(&mut self.muted_word);
                self.show(*meter, f64::from(*percent), *muted, label, now);
            }
            Message::Hide => {
                self.hide_at = None;
                self.aim_opacity(0.0, now);
            }
            Message::Ping => {}
            Message::Bye => {
                self.bye = Some(now);
                self.hide_at = None;
                self.aim_opacity(0.0, now);
            }
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

    /// The client process ended at `now`: exit unless another `Hello` arrives within
    /// `EXIT_GRACE_S`.
    pub fn client_gone(&mut self, now: f64) {
        self.arm_exit(now + EXIT_GRACE_S);
    }

    /// The client closed the pipe at `now`: exit unless another `Hello` arrives within
    /// `EXIT_GRACE_S`.
    pub fn disconnected(&mut self, now: f64) {
        self.arm_exit(now + EXIT_GRACE_S);
    }

    /// Arms the exit at `at`, keeping an earlier one.
    fn arm_exit(&mut self, at: f64) {
        self.exit_at = Some(self.exit_at.map_or(at, |armed| armed.min(at)));
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
            muted_word: self.muted_word.clone(),
        }
    }

    /// The label and muted word a prewarm draws: the last ones sent, else `"VOLUME"` and the
    /// English muted word.
    #[must_use]
    pub fn warm_words(&self) -> (&str, &str) {
        let label = if self.label.is_empty() {
            DEFAULT_LABEL
        } else {
            &self.label
        };
        let word = if self.muted_word.is_empty() {
            text(Text::Muted, Lang::En)
        } else {
            &self.muted_word
        };
        (label, word)
    }

    /// Whether the helper should exit at `now`: after `Bye` once hidden (or `BYE_FADE_S`
    /// after it), `EXIT_GRACE_S` after the client went away with no new `Hello`, or at
    /// `START_GRACE_S` when no `Hello` ever came.
    #[must_use]
    pub fn should_exit(&self, now: f64) -> bool {
        let bye_done = self
            .bye
            .is_some_and(|at| self.is_hidden() || now >= at + BYE_FADE_S);
        bye_done || self.exit_at.is_some_and(|at| now >= at)
    }

    /// The next time something is due while at rest (the hide, the exit or the end of the
    /// `Bye` fade), `None` when nothing is.
    #[must_use]
    pub fn deadline(&self) -> Option<f64> {
        [
            self.hide_at,
            self.exit_at,
            self.bye.map(|at| at + BYE_FADE_S),
        ]
        .into_iter()
        .flatten()
        .reduce(f64::min)
    }

    /// Whether the opacity is moving (a show or hide motion), as opposed to a level-only
    /// change or a repaint.
    #[must_use]
    pub fn is_fading(&self) -> bool {
        !self.opacity.is_settled()
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

    /// Whether a new graphics stack should draw its warm frames: only while hidden (the first
    /// build and hidden rebuilds). A rebuild while the OSD is on screen skips them, since the
    /// warm present would blink over the shown frame until the repaint.
    #[must_use]
    pub fn wants_prewarm(&self) -> bool {
        self.is_hidden()
    }
}

#[cfg(test)]
mod tests;
