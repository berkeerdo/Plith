//! The owner's side of a relayout: decides whether the clock must change and wakes the loop.

use super::run::{LoopShared, Relayout};
use plith_core::clock_choice::clock_order;
use plith_core::display::Display;
use plith_core::launch::ClockRequest;
use plith_core::scene::SceneMailbox;
use std::sync::{Arc, Mutex, PoisonError};

/// Sends display changes to the render thread; usable from any thread.
#[derive(Clone)]
pub struct RelayoutHandle {
    pub(super) mailbox: Arc<SceneMailbox>,
    pub(super) shared: Arc<LoopShared>,
    /// The clock the run asked for, to rebuild the order for a remote or local session.
    pub(super) clock: ClockRequest,
    /// The display the loop was last asked to follow.
    pub(super) last: Arc<Mutex<Display>>,
}

impl RelayoutHandle {
    /// Asks the loop to follow `next`; the clock is reopened only when the monitor, the rate
    /// or the remote state changed since the last display.
    pub fn relayout(&self, next: Display) {
        let mut last = self.last.lock().unwrap_or_else(PoisonError::into_inner);
        let order = last
            .clock_changed(&next)
            .then(|| clock_order(self.clock, next.remote));
        if order.is_some() {
            self.shared.set_clock_target(next.monitor, next.hz);
        }
        *last = next;
        drop(last);
        self.shared.request_relayout(Relayout {
            layout: next.layout,
            hz: next.hz,
            order,
        });
        let _ = self.mailbox.nudge();
    }
}
