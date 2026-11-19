//! The value a ruler drag shows before the system reads it back: while a finger is on the
//! volume, brightness or seek ruler, its page draws the dragged value instead of the reading.
//! The override ends when the reading catches up or one second after the last set.

use super::Feed;
use crate::pages::Dirty;
use crate::services::Facts;
use plith_core::input::drag::RulerKind;
use plith_core::pages::PageId;
use plith_core::scene::SceneMailbox;
use plith_shell::FIRST_APP_TIMER;
use std::time::SystemTime;

/// A one-shot that drops the override one second after its last set.
pub const DRAG_TIMER: usize = FIRST_APP_TIMER + 2;
/// Seconds an override outlives its last set when no reading catches up.
const OVERRIDE_LIFE_S: f64 = 1.0;
/// The timer period in milliseconds.
const OVERRIDE_LIFE_MS: u32 = 1000;
/// How close a reading must be to the dragged percent (volume, brightness) to take over.
const PERCENT_TOLERANCE: f64 = 1.0;
/// How close a reading must be to the dragged fraction (seek) to take over.
const FRACTION_TOLERANCE: f64 = 0.01;

/// The value a ruler was last dragged to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Override {
    kind: RulerKind,
    /// A percent for volume and brightness, a fraction of the track for seek.
    value: f64,
    set_at_s: f64,
    /// A press is still down: the override outlives its second until the release.
    held: bool,
    /// The expiry timer is running.
    armed: bool,
}

/// The page a ruler's value is drawn on.
fn page_of(kind: RulerKind) -> PageId {
    match kind {
        RulerKind::Volume => PageId::Audio,
        RulerKind::Brightness => PageId::System,
        RulerKind::Seek => PageId::Media,
    }
}

/// A dirty set naming only the page of `kind`.
fn dirty_of(kind: RulerKind) -> Dirty {
    Dirty::page(page_of(kind))
}

impl Override {
    /// `facts` with the dragged value in place of the reading.
    fn applied(self, facts: &Facts) -> Facts {
        let mut shown = facts.clone();
        match self.kind {
            RulerKind::Volume => {
                if let Some((volume, _)) = shown.audio.attached.as_mut() {
                    // Allowed: a percent of 0 to 100 fits f32 exactly enough.
                    #[allow(clippy::cast_possible_truncation)]
                    let level = (self.value / 100.0) as f32;
                    *volume = level;
                }
            }
            RulerKind::Brightness => {
                if let Some((_, level)) = shown.system.brightness.as_mut() {
                    // Allowed: the value is a rounded percent of 0 to 100.
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    let percent = self.value.round().clamp(0.0, 100.0) as u8;
                    *level = percent;
                }
            }
            RulerKind::Seek => {
                if let Some(timeline) = shown.media.timeline.as_mut() {
                    timeline.position_s = self.value * timeline.duration_s;
                    timeline.stamp = SystemTime::now();
                    // The fill holds still under the finger.
                    timeline.playing = false;
                }
            }
        }
        shown
    }

    /// True when the reading in `facts` is where the drag put the value.
    fn caught_up(self, facts: &Facts) -> bool {
        match self.kind {
            RulerKind::Volume => facts.audio.attached.is_some_and(|(v, _)| {
                (f64::from(v) * 100.0 - self.value).abs() <= PERCENT_TOLERANCE
            }),
            RulerKind::Brightness => facts
                .system
                .brightness
                .as_ref()
                .is_some_and(|(_, b)| (f64::from(*b) - self.value).abs() <= PERCENT_TOLERANCE),
            RulerKind::Seek => facts.media.timeline.is_some_and(|t| {
                t.duration_s > 0.0
                    && (t.position_s / t.duration_s - self.value).abs() <= FRACTION_TOLERANCE
            }),
        }
    }

    /// True when `other` would draw the same as this one.
    fn same_look(self, other: Override) -> bool {
        let epsilon = match self.kind {
            RulerKind::Seek => 0.001,
            _ => 0.5,
        };
        self.kind == other.kind && (self.value - other.value).abs() < epsilon
    }
}

impl Feed {
    /// The facts with the override applied, for building pages; `None` without one.
    pub(super) fn shown_facts(&self) -> Option<Facts> {
        self.drag.map(|o| o.applied(&self.facts))
    }

    /// Shows `value` (a percent, or a fraction for seek) on its ruler's page in place of the
    /// reading, and restarts the one second life of the override.
    pub fn set_override(
        &mut self,
        kind: RulerKind,
        value: f64,
        now_s: f64,
        mailbox: &SceneMailbox,
    ) {
        let next = Override {
            kind,
            value,
            set_at_s: now_s,
            held: true,
            armed: self.drag.is_some_and(|old| old.armed),
        };
        let moved = self.drag.is_none_or(|old| !old.same_look(next));
        let mut dirty = if moved {
            dirty_of(kind)
        } else {
            Dirty::default()
        };
        if let Some(old) = self.drag.filter(|old| old.kind != kind) {
            dirty = dirty.union(dirty_of(old.kind));
        }
        self.drag = Some(next);
        self.arm_expiry(OVERRIDE_LIFE_MS);
        self.rebuild(dirty, mailbox);
    }

    /// Starts the expiry timer unless it already runs: one timer per drag, not one per point.
    fn arm_expiry(&mut self, ms: u32) {
        if let Some(over) = self.drag.as_mut()
            && !over.armed
        {
            over.armed = true;
            self.timers.set(DRAG_TIMER, ms);
        }
    }

    /// The press ended: the override now lives one second more from `now_s`, then drops.
    pub fn end_hold(&mut self, now_s: f64) {
        if let Some(over) = self.drag.as_mut() {
            over.held = false;
            over.set_at_s = now_s;
        }
    }

    /// Ends the override once the reading in the facts caught up; returns the page to rebuild.
    pub(super) fn settle_override(&mut self) -> Dirty {
        match self.drag.filter(|o| o.caught_up(&self.facts)) {
            Some(over) => {
                self.drag = None;
                self.timers.kill(DRAG_TIMER);
                dirty_of(over.kind)
            }
            None => Dirty::default(),
        }
    }

    /// The override timer ticked: drops the override once a second passed since its last set.
    pub fn on_drag_timer(&mut self, now_s: f64, mailbox: &SceneMailbox) {
        self.timers.kill(DRAG_TIMER);
        if let Some(over) = self.drag.as_mut() {
            over.armed = false;
        }
        let Some(over) = self.drag else {
            return;
        };
        // The reading that lags the write must not snap the ruler back under a held press.
        let left = if over.held {
            OVERRIDE_LIFE_S
        } else {
            OVERRIDE_LIFE_S - (now_s - over.set_at_s)
        };
        if left > 1e-9 {
            // Allowed: `left` is at most one second.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let ms = (left * 1000.0).ceil().max(1.0) as u32;
            self.arm_expiry(ms);
            return;
        }
        self.drag = None;
        self.rebuild(dirty_of(over.kind), mailbox);
    }

    /// The ruler whose dragged value is shown now.
    #[cfg(test)]
    pub fn override_kind(&self) -> Option<RulerKind> {
        self.drag.map(|o| o.kind)
    }
}
