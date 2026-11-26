//! The Halka theme's open and collapse motion as pure data: fabric reveal, three ripples, the
//! source disc lift and the pill fade (spec `2026-11-26-halka-island-design.md`, section 5).
//!
//! The morph's `progress` (height between its start and target, so it follows the spring and
//! its retargets) drives the reveal, the disc lift, the pill and the ripple presses. A press is
//! latched: `advance` records it each frame and it never comes back out while expanding. Time
//! drives only the hold and fade after all three are pressed, and the collapse. All opacity
//! curves are linear ramps.
//!
//! The content gate (Berke, 2026-11-26): the page content of an open waits until its ripples
//! have pressed and faded to 0, so no frame shows ripples and page text together.
//! [`MaterialMotion::content_gate`] latches open in `advance` the first frame the ripples are
//! gone; the engine holds the content handover until then.

#![allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]

pub mod art;
mod line;
pub mod place;

pub use line::MorphLine;

/// The morph progress where each ripple starts pressing (Berke's pick, 2026-11-26).
pub const RIPPLE_FROM: [f64; 3] = [0.15, 0.30, 0.45];
/// A ripple presses (0 to 1) over this much progress.
pub const PRESS_SPAN: f64 = 0.15;
/// All three ripples are pressed from this progress on.
pub const ALL_PRESSED_AT: f64 = RIPPLE_FROM[2] + PRESS_SPAN;
/// The pressed ripples hold at 1 this long after the last one is pressed (0: the fade starts
/// at once, so the page waits as little as it can; 120 before 2026-11-26).
pub const HOLD_MS: f64 = 0.0;
/// The shared open fade (1 to 0) that starts when the hold ends (180 before 2026-11-26).
pub const FADE_MS: f64 = 120.0;
/// Collapse: each ripple fades (its value to 0) over this long.
pub const RISE_MS: f64 = 180.0;
/// Collapse: each ripple starts leaving this long after the previous one, in order 3, 2, 1.
pub const RISE_STEP_MS: f64 = 60.0;
/// The reveal circle reaches the far corner at this morph progress.
pub const REVEAL_FULL_AT: f64 = 0.55;
/// Progress where the disc starts to lift out of the shape.
pub const LIFT_FROM: f64 = 0.30;
/// Progress where the lift ends.
pub const LIFT_TO: f64 = 0.60;
/// The disc lifts this far, in DIP (linear in progress).
pub const LIFT_DIP: f64 = 40.0;
/// The pill is fully visible at progress 0 and gone at this progress.
pub const PILL_GONE_AT: f64 = 0.30;
/// A reopen during a collapse replays the ripples only if the reveal had shrunk below this.
pub const REPLAY_BELOW: f64 = 0.10;

/// Time after `collapse` when the last ripple (index 0) has faded out.
const COLLAPSE_DONE_MS: f64 = 2.0 * RISE_STEP_MS + RISE_MS;
/// Time after all three are pressed when the open's ripples are gone (the content gate opens).
const OPEN_DONE_MS: f64 = HOLD_MS + FADE_MS;
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
    /// Opening or open. Each ripple presses from its `base` (0 on a fresh open, the current
    /// value on a replay) toward 1 as the progress passes its span; `pressed` is the latched
    /// value, and `full_at` the time all three reached 1 (the hold starts there). `content`
    /// latches true on the first frame after the hold and fade (the content gate).
    Expanding {
        base: [f32; 3],
        pressed: [f32; 3],
        full_at: Option<f64>,
        content: bool,
    },
    /// `collapse` was called at `since`; ripples fade from the `from` values.
    Collapsing {
        since: f64,
        from: [f32; 3],
    },
    /// A reopen that does not replay: the fabric follows the spring again while the ripples keep
    /// fading from `from`, on the collapse schedule that began at `since`. `content` latches
    /// true once they are all 0 (at once when there were none).
    Reopened {
        since: f64,
        from: [f32; 3],
        content: bool,
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

/// Ripple `k` pressed from `base` at `progress`; exactly `base` before its span and exactly 1
/// at its end.
fn press(k: usize, progress: f64, base: f32) -> f32 {
    let r = ramp(progress, RIPPLE_FROM[k], PRESS_SPAN) as f32;
    if r >= 1.0 {
        1.0
    } else {
        base + (1.0 - base) * r
    }
}

/// The shared fade factor (1 while holding, 0 when faded) `now_ms` after `full_at`.
fn hold_fade(now_ms: f64, full_at: Option<f64>) -> f32 {
    full_at.map_or(1.0, |t| 1.0 - ramp(now_ms, t + HOLD_MS, FADE_MS) as f32)
}

/// Ripple values `t` ms into a collapse. Ripple 3 (index 2) leaves first. "Rise" is read as a
/// fade from the current value to 0 over `RISE_MS`, delayed by `RISE_STEP_MS` per step, so the
/// layers right after `collapse` equal the layers right before (no jump).
fn collapse_ripples(from: [f32; 3], t: f64) -> [f32; 3] {
    [0, 1, 2].map(|k| {
        let delay = (2 - k) as f64 * RISE_STEP_MS;
        from[k] * (1.0 - ramp(t, delay, RISE_MS)) as f32
    })
}

fn all_zero(ripples: [f32; 3]) -> bool {
    ripples.iter().all(|r| *r <= 0.0)
}

impl MaterialMotion {
    /// A timeline at rest, collapsed.
    #[must_use]
    pub fn new() -> Self {
        Self {
            phase: Phase::Collapsed,
        }
    }

    /// A timeline at rest, open: pressed and faded long ago (an island that jumped open with no
    /// motion, such as a suppressed scene).
    #[must_use]
    pub fn open() -> Self {
        Self {
            phase: Phase::Expanding {
                base: [0.0; 3],
                pressed: [1.0; 3],
                full_at: Some(f64::NEG_INFINITY),
                content: true,
            },
        }
    }

    fn ripples_at(&self, now_ms: f64) -> [f32; 3] {
        match self.phase {
            Phase::Collapsed => [0.0; 3],
            Phase::Expanding {
                pressed, full_at, ..
            } => {
                let fade = hold_fade(now_ms, full_at);
                pressed.map(|p| p * fade)
            }
            Phase::Collapsing { since, from } | Phase::Reopened { since, from, .. } => {
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
                    base: [0.0; 3],
                    pressed: [0.0; 3],
                    full_at: None,
                    content: false,
                }
            }
            Phase::Collapsing { since, from } => {
                let reveal = (progress / REVEAL_FULL_AT).clamp(0.0, 1.0);
                let left = collapse_ripples(from, now_ms - since);
                self.phase = if reveal < REPLAY_BELOW {
                    Phase::Expanding {
                        base: left,
                        pressed: left,
                        full_at: None,
                        content: false,
                    }
                } else {
                    Phase::Reopened {
                        since,
                        from,
                        content: all_zero(left),
                    }
                };
            }
        }
    }

    /// Records the frame at `now_ms` and `progress`: while expanding, each ripple's press is
    /// latched at its highest value so far, and the hold starts the first time all three are
    /// pressed; the content gate latches open on the first frame the ripples are gone. Call it
    /// every frame before [`Self::layers`].
    pub fn advance(&mut self, now_ms: f64, progress: f64) {
        match &mut self.phase {
            Phase::Expanding {
                base,
                pressed,
                full_at,
                content,
            } => {
                for (k, p) in pressed.iter_mut().enumerate() {
                    *p = p.max(press(k, progress, base[k]));
                }
                if full_at.is_none() && pressed.iter().all(|p| *p >= 1.0) {
                    *full_at = Some(now_ms);
                }
                *content |= full_at.is_some_and(|t| now_ms - t >= OPEN_DONE_MS);
            }
            Phase::Reopened {
                since,
                from,
                content,
            } => *content |= all_zero(collapse_ripples(*from, now_ms - *since)),
            Phase::Collapsed | Phase::Collapsing { .. } => {}
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

    /// The layers at `now_ms` for the morph's current `progress` (0..1). The ripples are the
    /// latched presses as of the last [`Self::advance`].
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

    /// The content gate: false while an open still has ripples to press or fade, so its page
    /// content must wait (it latches open in [`Self::advance`] on the first frame they are all
    /// 0, and stays open through a page switch). Always true while collapsed or collapsing.
    #[must_use]
    pub fn content_gate(&self) -> bool {
        match self.phase {
            Phase::Expanding { content, .. } | Phase::Reopened { content, .. } => content,
            Phase::Collapsed | Phase::Collapsing { .. } => true,
        }
    }

    /// True when nothing is animating: ripples done (and, while open, the content gate latched
    /// open by an [`Self::advance`]) and the progress at the end of the direction.
    #[must_use]
    pub fn is_settled(&self, now_ms: f64, progress: f64) -> bool {
        match self.phase {
            Phase::Collapsed => progress <= PROGRESS_EPS,
            Phase::Expanding { content, .. } => content && progress >= 1.0 - PROGRESS_EPS,
            Phase::Reopened { since, content, .. } => {
                content && now_ms - since >= COLLAPSE_DONE_MS && progress >= 1.0 - PROGRESS_EPS
            }
            Phase::Collapsing { since, .. } => {
                now_ms - since >= COLLAPSE_DONE_MS && progress <= PROGRESS_EPS
            }
        }
    }
}

#[cfg(test)]
mod tests;
