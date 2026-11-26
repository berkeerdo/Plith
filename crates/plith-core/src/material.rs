//! The Halka theme's open and collapse motion as pure data: fabric reveal, three ripples, the
//! source disc lift and the pill fade (spec `2026-11-26-halka-island-design.md`, section 5).
//!
//! Two clocks drive it. The reveal, the disc lift and the pill follow the morph's `progress`
//! (height between its start and target), so they follow the spring and its retargets. The
//! ripples run on time since `expand`. All opacity curves are linear ramps.

#![allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]

/// The measured `brisk` expand full time in milliseconds: the visible/full pair "expand 417/592 ms"
/// in `docs/plans/done/engine-r1b.md` (line 379); 592 is the full time.
pub const EXPAND_SETTLE_MS: f64 = 592.0;
/// When each ripple starts pressing, in ms since `expand`: 25, 40 and 55 % of the settle time.
pub const RIPPLE_START_MS: [f64; 3] = [
    EXPAND_SETTLE_MS * 0.25,
    EXPAND_SETTLE_MS * 0.40,
    EXPAND_SETTLE_MS * 0.55,
];
/// A ripple presses (0 to 1) over this long.
pub const PRESS_MS: f64 = 90.0;
/// A pressed ripple holds at 1 this long. Ripples 1 and 2 keep holding until the last ripple's
/// hold ends, so that all three fade together.
pub const HOLD_MS: f64 = 120.0;
/// The shared fade (1 to 0) that starts when the last ripple's hold ends.
pub const FADE_MS: f64 = 180.0;
/// Collapse: each ripple starts leaving this long after the previous one, in order 3, 2, 1.
pub const RISE_STEP_MS: f64 = 60.0;
/// The reveal circle reaches the far corner at this morph progress.
pub const REVEAL_FULL_AT: f64 = 0.55;
/// The disc lifts out of the shape between these two progress values.
/// Progress where the lift starts.
pub const LIFT_FROM: f64 = 0.30;
/// Progress where the lift ends.
pub const LIFT_TO: f64 = 0.60;
/// The disc lifts this far, in DIP (linear in progress).
pub const LIFT_DIP: f64 = 40.0;
/// The pill is fully visible at progress 0 and gone at this progress.
pub const PILL_GONE_AT: f64 = 0.30;
/// A reopen during a collapse replays the ripples only if the reveal had shrunk below this.
pub const REPLAY_BELOW: f64 = 0.10;

/// Time after `expand` when the last ripple has faded out.
const EXPAND_DONE_MS: f64 = RIPPLE_START_MS[2] + PRESS_MS + HOLD_MS + FADE_MS;
/// Time after `collapse` when the last ripple (index 0) has faded out.
const COLLAPSE_DONE_MS: f64 = 2.0 * RISE_STEP_MS + FADE_MS;
/// Progress tolerance for "at the end" in `is_settled`.
const PROGRESS_EPS: f64 = 1e-3;

/// Everything the painter needs for one frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MaterialLayers {
    /// Fabric reveal radius, 0..1 of the far-corner radius.
    pub reveal: f64,
    /// Ripple opacities, 0..1.
    pub ripples: [f32; 3],
    /// Source disc opacity.
    pub disc: f32,
    /// How far the source disc has lifted, in DIP up.
    pub disc_lift: f64,
    /// Collapsed pill opacity.
    pub pill: f32,
}

impl MaterialLayers {
    /// Collapsed: the pill only.
    pub const COLLAPSED: Self = Self {
        reveal: 0.0,
        ripples: [0.0; 3],
        disc: 0.0,
        disc_lift: 0.0,
        pill: 1.0,
    };
    /// Open at rest: the fabric fully revealed, the disc gone.
    pub const OPEN: Self = Self {
        reveal: 1.0,
        ripples: [0.0; 3],
        disc: 0.0,
        disc_lift: LIFT_DIP,
        pill: 0.0,
    };
}

/// Morph progress from the current height: 0 at `rest`, 1 at `open`, clamped; 1 when `open <= rest`.
#[must_use]
pub fn morph_progress(height: f64, rest: f64, open: f64) -> f64 {
    if open <= rest {
        return 1.0;
    }
    ((height - rest) / (open - rest)).clamp(0.0, 1.0)
}

#[derive(Clone, Copy, Debug)]
enum Phase {
    Collapsed,
    /// `expand` was called at `since`; the ripples run on `now - since`, each pressing from its
    /// `base` value (0 on a fresh open, the current value on a replay) toward 1.
    Expanding {
        since: f64,
        base: [f32; 3],
    },
    /// `collapse` was called at `since`; ripples fade from the `from` values.
    Collapsing {
        since: f64,
        from: [f32; 3],
    },
    /// A reopen that does not replay: the fabric follows the spring again while the ripples keep
    /// fading from `from`, on the collapse schedule that began at `since`.
    Reopened {
        since: f64,
        from: [f32; 3],
    },
}

/// The material timeline state.
#[derive(Debug)]
pub struct MaterialMotion {
    phase: Phase,
}

impl Default for MaterialMotion {
    fn default() -> Self {
        Self::new()
    }
}

fn ramp(t: f64, from: f64, len: f64) -> f64 {
    ((t - from) / len).clamp(0.0, 1.0)
}

/// Ripple `k` while expanding, `t` ms since `expand`: press from `base`, hold until the last
/// ripple's hold ends, then the shared fade.
fn expand_ripple(k: usize, t: f64, base: f32) -> f32 {
    let press = base + (1.0 - base) * ramp(t, RIPPLE_START_MS[k], PRESS_MS) as f32;
    let fade_from = RIPPLE_START_MS[2] + PRESS_MS + HOLD_MS;
    press * (1.0 - ramp(t, fade_from, FADE_MS)) as f32
}

/// Ripple values `t` ms into a collapse. Ripple 3 (index 2) leaves first. "Rise" is read as a
/// fade from the current value to 0 over `FADE_MS`, delayed by `RISE_STEP_MS` per step, so the
/// layers right after `collapse` equal the layers right before (no jump).
fn collapse_ripples(from: [f32; 3], t: f64) -> [f32; 3] {
    [0, 1, 2].map(|k| {
        let delay = (2 - k) as f64 * RISE_STEP_MS;
        from[k] * (1.0 - ramp(t, delay, FADE_MS)) as f32
    })
}

impl MaterialMotion {
    /// A timeline at rest, collapsed.
    #[must_use]
    pub fn new() -> Self {
        Self {
            phase: Phase::Collapsed,
        }
    }

    fn ripples_at(&self, now_ms: f64) -> [f32; 3] {
        match self.phase {
            Phase::Collapsed => [0.0; 3],
            Phase::Expanding { since, base } => {
                let t = now_ms - since;
                [0, 1, 2].map(|k| expand_ripple(k, t, base[k]))
            }
            Phase::Collapsing { since, from } | Phase::Reopened { since, from } => {
                collapse_ripples(from, now_ms - since)
            }
        }
    }

    /// Start (or restart) the open sequence; `progress` is the morph progress right now. While
    /// open or opening this is a no-op, so a page switch replays nothing. During a collapse the
    /// ripples replay only if the reveal had shrunk below `REPLAY_BELOW`; either way they
    /// continue from their current opacities (a replay presses up from them, never down; no
    /// replay keeps fading them out), so nothing jumps.
    pub fn expand(&mut self, now_ms: f64, progress: f64) {
        match self.phase {
            Phase::Expanding { .. } | Phase::Reopened { .. } => {}
            Phase::Collapsed => {
                self.phase = Phase::Expanding {
                    since: now_ms,
                    base: [0.0; 3],
                }
            }
            Phase::Collapsing { since, from } => {
                let reveal = (progress / REVEAL_FULL_AT).clamp(0.0, 1.0);
                self.phase = if reveal < REPLAY_BELOW {
                    Phase::Expanding {
                        since: now_ms,
                        base: collapse_ripples(from, now_ms - since),
                    }
                } else {
                    Phase::Reopened { since, from }
                };
            }
        }
    }

    /// Start the collapse sequence from the layers' current ripple opacities.
    pub fn collapse(&mut self, now_ms: f64) {
        if let Phase::Collapsed | Phase::Collapsing { .. } = self.phase {
            return;
        }
        let from = self.ripples_at(now_ms);
        self.phase = Phase::Collapsing {
            since: now_ms,
            from,
        };
    }

    /// The layers at `now_ms` for the morph's current `progress` (0..1).
    ///
    /// Lift is linear in progress over `LIFT_FROM..LIFT_TO`. The pill fades linearly to 0 at
    /// `PILL_GONE_AT`. The disc fades in as the pill fades out (`1 - pill`) and fades out as it
    /// lifts, so it is 1 at the end of the pill fade, 0 once lifted out, and 0 at rest.
    #[must_use]
    pub fn layers(&self, now_ms: f64, progress: f64) -> MaterialLayers {
        let lift = ramp(progress, LIFT_FROM, LIFT_TO - LIFT_FROM);
        let pill = 1.0 - ramp(progress, 0.0, PILL_GONE_AT);
        MaterialLayers {
            reveal: (progress / REVEAL_FULL_AT).clamp(0.0, 1.0),
            ripples: self.ripples_at(now_ms),
            disc: ((1.0 - pill) * (1.0 - lift)) as f32,
            disc_lift: LIFT_DIP * lift,
            pill: pill as f32,
        }
    }

    /// True when nothing is animating: ripples done and the progress at the end of the direction.
    #[must_use]
    pub fn is_settled(&self, now_ms: f64, progress: f64) -> bool {
        match self.phase {
            Phase::Collapsed => progress <= PROGRESS_EPS,
            Phase::Expanding { since, .. } => {
                now_ms - since >= EXPAND_DONE_MS && progress >= 1.0 - PROGRESS_EPS
            }
            Phase::Reopened { since, .. } => {
                now_ms - since >= COLLAPSE_DONE_MS && progress >= 1.0 - PROGRESS_EPS
            }
            Phase::Collapsing { since, .. } => {
                now_ms - since >= COLLAPSE_DONE_MS && progress <= PROGRESS_EPS
            }
        }
    }
}

#[cfg(test)]
mod tests;
