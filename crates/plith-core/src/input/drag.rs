//! Dragging a ruler: which ruler a press began on, the value at a pointer x, and the write
//! throttle that keeps a fast drag from flooding the system with calls.

use crate::draw::Rect;
use crate::pages::{Action, HitTarget, seek_fraction, system, target_at};

/// Minimum seconds between two system writes during a drag (30 per second).
pub const DRAG_WRITE_GAP_S: f64 = 1.0 / 30.0;

/// Which value a ruler sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RulerKind {
    /// The volume of the default output.
    Volume,
    /// The brightness of the system monitor.
    Brightness,
    /// The playback position of the current track.
    Seek,
}

/// A drag that began on a ruler.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RulerDrag {
    /// What the ruler sets.
    pub kind: RulerKind,
    /// The ruler's rect in page DIP.
    pub ruler: Rect,
}

/// The ruler under the point, with the same 4 DIP slack as `hit`; `None` for any other target.
#[must_use]
pub fn begin_drag(targets: &[HitTarget], x: f64, y: f64) -> Option<RulerDrag> {
    let target = target_at(targets, x, y)?;
    let (kind, ruler) = match &target.action {
        Action::VolumeRuler(r) => (RulerKind::Volume, *r),
        Action::BrightnessRuler(r) => (RulerKind::Brightness, *r),
        Action::SeekRuler(r) => (RulerKind::Seek, *r),
        _ => return None,
    };
    Some(RulerDrag { kind, ruler })
}

impl RulerDrag {
    /// The action for the pointer at page x, clamped to the ruler's ends.
    #[must_use]
    pub fn value_at(&self, x: f64) -> Action {
        match self.kind {
            RulerKind::Volume => Action::SetVolume(system::brightness_at(self.ruler, x)),
            RulerKind::Brightness => Action::SetBrightness(system::brightness_at(self.ruler, x)),
            RulerKind::Seek => Action::Seek(seek_fraction(self.ruler, x)),
        }
    }
}

/// Lets one event through per minimum gap.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Throttle {
    min_gap_s: f64,
    last_due: Option<f64>,
}

impl Throttle {
    /// A throttle that passes at most one call per `min_gap_s` seconds.
    #[must_use]
    pub fn new(min_gap_s: f64) -> Self {
        Self {
            min_gap_s,
            last_due: None,
        }
    }

    /// True on the first call, then only once `min_gap_s` has passed since the last true.
    pub fn due(&mut self, now_s: f64) -> bool {
        let ready = self
            .last_due
            .is_none_or(|last| now_s - last >= self.min_gap_s - 1e-9);
        if ready {
            self.last_due = Some(now_s);
        }
        ready
    }
}

#[cfg(test)]
mod tests;
