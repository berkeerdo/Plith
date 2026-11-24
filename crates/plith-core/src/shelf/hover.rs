//! The drag hover state machine of ADR 0018. A file drag over the island for 150 ms opens the
//! shelf; leaving starts a 380 ms grace before the normal collapse; the island's own drag out
//! never counts. Pure: the caller passes the time in seconds and arms one timer from the
//! returned [`Cmd`].

/// How long a drag must stay over the island before the shelf opens, in seconds.
pub const DWELL_S: f64 = 0.150;
/// How long the shelf stays open after the drag left, in seconds.
pub const GRACE_S: f64 = 0.380;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
enum Phase {
    #[default]
    Idle,
    /// Over the island, the dwell has not run out.
    Dwell { since: f64 },
    /// The shelf is open for the drag.
    Open { since: f64 },
    /// The drag left; the shelf waits for it to come back.
    Grace { since: f64 },
}

/// What the caller does after a step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Cmd {
    /// Nothing.
    Nothing,
    /// Check again with [`DragHover::tick`] after this many seconds.
    Wait(f64),
    /// Open the island on the shelf page. `ms` is the time from the first hover.
    Open {
        /// Milliseconds from the first hover to the open.
        ms: u32,
    },
    /// The grace ran out: let the island collapse the normal way.
    Collapse,
}

/// The drag hover state.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DragHover {
    phase: Phase,
    own: bool,
}

fn millis(seconds: f64) -> u32 {
    // Allowed: a hover time is a few hundred milliseconds, far inside u32.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let ms = (seconds.max(0.0) * 1000.0).round() as u32;
    ms
}

impl DragHover {
    /// Marks the island's own drag out as running (`true`) or over. While it runs every other
    /// call does nothing: the shelf must not take its own wagons back in.
    pub fn set_own(&mut self, own: bool) {
        self.own = own;
        if own {
            self.phase = Phase::Idle;
        }
    }

    /// True while a file drag holds the island open.
    #[must_use]
    pub fn hovering(&self) -> bool {
        matches!(self.phase, Phase::Dwell { .. } | Phase::Open { .. })
    }

    /// True while the slot should be drawn hot: the shelf is open for a drag that is over it.
    #[must_use]
    pub fn hot(&self) -> bool {
        matches!(self.phase, Phase::Open { .. })
    }

    /// True while any phase but idle runs (a timer may be needed).
    #[must_use]
    pub fn active(&self) -> bool {
        self.phase != Phase::Idle
    }

    /// A file drag is over the island at `now`. Repeated calls while it stays are cheap.
    pub fn over(&mut self, now: f64) -> Cmd {
        if self.own {
            return Cmd::Nothing;
        }
        match self.phase {
            Phase::Idle => {
                self.phase = Phase::Dwell { since: now };
                Cmd::Wait(DWELL_S)
            }
            Phase::Dwell { .. } => self.tick(now),
            Phase::Open { .. } => Cmd::Nothing,
            Phase::Grace { since } => {
                self.phase = Phase::Open { since };
                Cmd::Nothing
            }
        }
    }

    /// The drag left the island at `now`.
    pub fn leave(&mut self, now: f64) -> Cmd {
        if self.own {
            return Cmd::Nothing;
        }
        match self.phase {
            Phase::Open { .. } => {
                self.phase = Phase::Grace { since: now };
                Cmd::Wait(GRACE_S)
            }
            Phase::Dwell { .. } => {
                self.phase = Phase::Idle;
                Cmd::Nothing
            }
            Phase::Idle | Phase::Grace { .. } => Cmd::Nothing,
        }
    }

    /// The files were dropped at `now`. The drag is over; a drop before the dwell ran out still
    /// opens the shelf so the new wagon can be seen.
    pub fn dropped(&mut self, now: f64) -> Cmd {
        if self.own {
            return Cmd::Nothing;
        }
        let was = std::mem::take(&mut self.phase);
        match was {
            Phase::Dwell { since } => Cmd::Open {
                ms: millis(now - since),
            },
            _ => Cmd::Nothing,
        }
    }

    /// The timer ran out at `now`: the dwell opens the shelf, the grace collapses it.
    pub fn tick(&mut self, now: f64) -> Cmd {
        match self.phase {
            Phase::Dwell { since } if now - since >= DWELL_S - 1e-9 => {
                self.phase = Phase::Open { since };
                Cmd::Open {
                    ms: millis(now - since),
                }
            }
            Phase::Dwell { since } => Cmd::Wait(DWELL_S - (now - since)),
            Phase::Grace { since } if now - since >= GRACE_S - 1e-9 => {
                self.phase = Phase::Idle;
                Cmd::Collapse
            }
            Phase::Grace { since } => Cmd::Wait(GRACE_S - (now - since)),
            Phase::Idle | Phase::Open { .. } => Cmd::Nothing,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_drag_that_only_crosses_opens_nothing() {
        let mut hover = DragHover::default();
        assert_eq!(hover.over(1.0), Cmd::Wait(DWELL_S));
        assert!(hover.hovering() && !hover.hot());
        assert_eq!(hover.leave(1.05), Cmd::Nothing);
        assert!(!hover.active());
        assert_eq!(hover.tick(1.2), Cmd::Nothing);
    }

    #[test]
    fn the_dwell_opens_the_shelf_once() {
        let mut hover = DragHover::default();
        hover.over(1.0);
        assert!(matches!(hover.tick(1.14), Cmd::Wait(w) if (w - 0.01).abs() < 1e-9));
        assert_eq!(hover.tick(1.152), Cmd::Open { ms: 152 });
        assert!(hover.hot());
        assert_eq!(hover.tick(1.5), Cmd::Nothing);
        assert_eq!(hover.over(1.6), Cmd::Nothing);
    }

    #[test]
    fn an_over_after_the_dwell_opens_without_waiting_for_the_timer() {
        let mut hover = DragHover::default();
        hover.over(1.0);
        assert_eq!(hover.over(1.2), Cmd::Open { ms: 200 });
    }

    #[test]
    fn leaving_an_open_shelf_starts_the_grace_then_collapses() {
        let mut hover = DragHover::default();
        hover.over(1.0);
        hover.tick(1.15);
        assert_eq!(hover.leave(2.0), Cmd::Wait(GRACE_S));
        assert!(!hover.hot() && hover.active());
        assert!(matches!(hover.tick(2.2), Cmd::Wait(_)));
        assert_eq!(hover.tick(2.38), Cmd::Collapse);
        assert!(!hover.active());
    }

    #[test]
    fn coming_back_inside_the_grace_keeps_the_shelf_open() {
        let mut hover = DragHover::default();
        hover.over(1.0);
        hover.tick(1.15);
        hover.leave(2.0);
        assert_eq!(hover.over(2.2), Cmd::Nothing);
        assert!(hover.hot());
        assert_eq!(hover.tick(2.6), Cmd::Nothing);
    }

    #[test]
    fn a_drop_ends_the_drag_and_leaves_the_collapse_to_the_pointer() {
        let mut hover = DragHover::default();
        hover.over(1.0);
        hover.tick(1.15);
        assert_eq!(hover.dropped(1.8), Cmd::Nothing);
        assert!(!hover.active());
    }

    #[test]
    fn a_drop_before_the_dwell_still_opens_the_shelf() {
        let mut hover = DragHover::default();
        hover.over(1.0);
        assert_eq!(hover.dropped(1.05), Cmd::Open { ms: 50 });
        assert!(!hover.active());
    }

    #[test]
    fn the_own_drag_is_ignored_in_every_call() {
        let mut hover = DragHover::default();
        hover.set_own(true);
        assert_eq!(hover.over(1.0), Cmd::Nothing);
        assert_eq!(hover.tick(2.0), Cmd::Nothing);
        assert_eq!(hover.leave(2.0), Cmd::Nothing);
        assert_eq!(hover.dropped(2.0), Cmd::Nothing);
        assert!(!hover.active());
        hover.set_own(false);
        assert_eq!(hover.over(3.0), Cmd::Wait(DWELL_S));
    }

    #[test]
    fn the_own_drag_flag_ends_a_hover_that_was_running() {
        let mut hover = DragHover::default();
        hover.over(1.0);
        hover.set_own(true);
        assert!(!hover.hovering());
    }
}
