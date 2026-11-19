//! The pure press state machine: a left press inside the island is a click until the pointer
//! moves past the drag slop, then a drag that ends in a release wherever the pointer is.

use plith_core::input::DRAG_SLOP;
use std::cell::Cell;

/// An island DIP point.
pub(crate) type Point = (f64, f64);

/// How a press ended.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum PressEnd {
    /// No press was recorded, or it ended outside the shape without a drag.
    Nothing,
    /// The press never passed the slop and was released inside the shape.
    Click(Point),
    /// The press was a drag: released at `at` (not clamped to the shape). `leave` is true when
    /// the release lies outside the shape: the window region is the shape, so the pointer has
    /// left the window whether or not a `WM_MOUSELEAVE` arrived during the capture.
    Release { at: Point, leave: bool },
}

/// A press that is waiting for its release.
#[derive(Debug, Clone, Copy)]
struct Active {
    origin: Point,
    /// The last drag point emitted, once the press has passed the slop.
    dragged: Option<Point>,
    /// A `WM_MOUSELEAVE` arrived during the drag; a lost capture then ends with a leave (a
    /// button-up decides by its own point).
    leave_pending: bool,
}

/// The press half of a click or drag, kept until the release, a lost capture or a leave.
#[derive(Debug, Default)]
pub(crate) struct PressRecord(Cell<Option<Active>>);

impl PressRecord {
    /// Records a press at `point` (`None`: outside the shape, which clears any record).
    /// Returns `true` when the press counts and the window should take the capture.
    pub(crate) fn down(&self, point: Option<Point>) -> bool {
        self.0.set(point.map(|origin| Active {
            origin,
            dragged: None,
            leave_pending: false,
        }));
        point.is_some()
    }

    /// Notes the pointer at the unclamped island point `raw`. Returns the drag point to report:
    /// once the distance from the press passes the slop, every new point.
    pub(crate) fn moved(&self, raw: Point) -> Option<Point> {
        let mut active = self.0.get()?;
        let (dx, dy) = (raw.0 - active.origin.0, raw.1 - active.origin.1);
        if active.dragged.is_none() && dx.hypot(dy) <= DRAG_SLOP {
            return None;
        }
        if active.dragged == Some(raw) {
            return None;
        }
        active.dragged = Some(raw);
        self.0.set(Some(active));
        Some(raw)
    }

    /// Ends the press at `raw` (unclamped); `inside` is the same point when it lies inside the
    /// shape. The record is always cleared.
    pub(crate) fn up(&self, inside: Option<Point>, raw: Point) -> PressEnd {
        match self.0.take() {
            None => PressEnd::Nothing,
            Some(active) if active.dragged.is_some() => PressEnd::Release {
                at: raw,
                leave: inside.is_none(),
            },
            Some(_) => inside.map_or(PressEnd::Nothing, PressEnd::Click),
        }
    }

    /// A leave arrived. Returns `true` while dragging: the drag goes on and the leave is
    /// remembered for the release. Otherwise the press is forgotten and `false` is returned.
    pub(crate) fn leave(&self) -> bool {
        match self.0.get() {
            Some(mut active) if active.dragged.is_some() => {
                active.leave_pending = true;
                self.0.set(Some(active));
                true
            }
            _ => {
                self.0.set(None);
                false
            }
        }
    }

    /// The capture was lost. A drag ends at its last point: returns that point and whether to
    /// leave, which is when a leave is pending or `inside` says the point lies outside the
    /// shape. A press that never dragged is forgotten.
    pub(crate) fn lost(&self, inside: impl FnOnce(Point) -> bool) -> Option<(Point, bool)> {
        let active = self.0.take()?;
        active
            .dragged
            .map(|at| (at, active.leave_pending || !inside(at)))
    }

    /// Forgets the press.
    #[cfg(test)]
    pub(crate) fn cancel(&self) {
        self.0.set(None);
    }
}
