//! Which frames belong to a logged motion: only opacity fades (show or hide) are motions;
//! a repaint or a level-only move is drawn without one.

use plith_core::motion::MotionKind;

/// What to do with the motion statistics around one frame.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Decision {
    /// Finish and log the running motion before this frame.
    pub end_before: bool,
    /// Start a motion of this kind with this frame.
    pub start: Option<MotionKind>,
    /// Record this frame in the running motion.
    pub record: bool,
    /// Finish and log the running motion after this frame.
    pub end_after: bool,
}

/// The kind of the motion being recorded, if any.
#[derive(Debug, Default)]
pub struct MotionTrack {
    current: Option<MotionKind>,
}

impl MotionTrack {
    /// Decides for a frame whose model shows `kind` and whose opacity is `fading` after the
    /// step.
    pub fn frame(&mut self, kind: MotionKind, fading: bool) -> Decision {
        let mut decision = Decision::default();
        if self.current.is_some_and(|running| running != kind) {
            decision.end_before = true;
            self.current = None;
        }
        if self.current.is_none() {
            if !fading {
                return decision;
            }
            decision.start = Some(kind);
            self.current = Some(kind);
        }
        decision.record = true;
        if !fading {
            decision.end_after = true;
            self.current = None;
        }
        decision
    }

    /// Ends the running motion; true when one was open.
    pub fn end(&mut self) -> bool {
        self.current.take().is_some()
    }

    /// Whether a motion is being recorded.
    #[cfg(test)]
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.current.is_some()
    }
}

#[cfg(test)]
mod tests;
