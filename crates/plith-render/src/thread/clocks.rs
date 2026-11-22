//! The frame clock in use: opened lazily, demoted down the preference order on failure.

use crate::clock::{ClockError, FrameSource};
use plith_core::clock_choice::{ClockHealth, ClockKind};

/// The clock in use, opened lazily, and the demotion state.
pub(super) struct Clocks<C, O> {
    open: O,
    pub(super) order: Vec<ClockKind>,
    index: usize,
    current: Option<C>,
    pub(super) health: ClockHealth,
}

impl<C: FrameSource, O: FnMut(ClockKind) -> Result<C, ClockError>> Clocks<C, O> {
    /// Clocks that open kinds from `order` with `open`, none opened yet.
    pub(super) fn new(open: O, order: Vec<ClockKind>) -> Self {
        Self {
            open,
            order,
            index: 0,
            current: None,
            health: ClockHealth::default(),
        }
    }

    pub(super) fn kind(&self) -> ClockKind {
        self.order[self.index.min(self.order.len() - 1)]
    }

    pub(super) fn is_last(&self) -> bool {
        self.index + 1 >= self.order.len()
    }

    /// Moves to the next kind; false (and no change) when this is the last one.
    pub(super) fn demote(&mut self) -> bool {
        if self.is_last() {
            return false;
        }
        self.current = None;
        self.index += 1;
        self.health = ClockHealth::default();
        true
    }

    /// Back to the preferred kind with fresh health, so a transient failure in one motion
    /// does not demote the clock for every later motion.
    pub(super) fn restart(&mut self) {
        if self.index != 0 {
            self.current = None;
            self.index = 0;
        }
        self.health = ClockHealth::default();
    }

    /// Drops the open clock and tries `order` from its best kind: the display changed, so the
    /// next source opens for the new monitor and rate.
    /// An empty `order` keeps the old one, which is never empty.
    pub(super) fn replace_order(&mut self, order: Vec<ClockKind>) {
        if !order.is_empty() {
            self.order = order;
        }
        self.current = None;
        self.index = 0;
        self.health = ClockHealth::default();
    }

    /// The current source, opening kinds in order until one opens.
    pub(super) fn source(&mut self) -> Result<&mut C, String> {
        while self.current.is_none() {
            let kind = self.kind();
            match (self.open)(kind) {
                Ok(source) => self.current = Some(source),
                Err(error) if self.demote() => drop(error),
                Err(error) => return Err(format!("no frame clock: {} {error}", kind.as_str())),
            }
        }
        self.current
            .as_mut()
            .ok_or_else(|| String::from("no frame clock"))
    }
}
