//! A display change reaches the loop beside the mailbox: the owner stores a [`Relayout`] in
//! [`LoopShared`] and nudges the mailbox; the loop takes it before its next frame, rebuilds the
//! paint stack at the new layout and, when the display's clock changed, reopens the clock.

use super::super::Paint;
use super::LoopShared;
use super::engine::Engine;
use super::prewarm::Reason;
use crate::clock::{ClockError, FrameSource};
use plith_core::clock_choice::ClockKind;
use plith_core::layout::SurfaceLayout;
use plith_core::theme::Palette;
use plith_core::timeline::VsyncTimeline;
use std::sync::{Mutex, MutexGuard, PoisonError};

/// A new layout for the loop, and the clock to use when the display's clock changed.
#[derive(Debug, Clone, PartialEq)]
pub struct Relayout {
    /// The surface layout to paint for.
    pub layout: SurfaceLayout,
    /// The nominal refresh rate of the display.
    pub hz: f64,
    /// The clock kinds to try, best first; `None` keeps the clock in use (and `hz` is then
    /// the rate it already has).
    pub order: Option<Vec<ClockKind>>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl LoopShared {
    /// Stores `next` for the loop. A request still pending is replaced, except that its clock
    /// change survives a later request that keeps the clock.
    pub fn request_relayout(&self, mut next: Relayout) {
        let mut slot = lock(&self.relayout);
        if next.order.is_none()
            && let Some(pending) = slot.take()
            && pending.order.is_some()
        {
            next.order = pending.order;
            next.hz = pending.hz;
        }
        *slot = Some(next);
    }

    /// Takes the pending relayout, if any.
    pub fn take_relayout(&self) -> Option<Relayout> {
        lock(&self.relayout).take()
    }

    /// Stores `palette` for the loop; a palette still pending is replaced.
    pub fn request_palette(&self, palette: Palette) {
        *lock(&self.palette) = Some(palette);
    }

    /// Takes the pending palette, if any.
    pub fn take_palette(&self) -> Option<Palette> {
        lock(&self.palette).take()
    }

    /// Sets the monitor and rate the next clock opens for.
    pub fn set_clock_target(&self, monitor: isize, hz: f64) {
        *lock(&self.clock_target) = (monitor, hz);
    }

    /// The monitor and rate the next clock opens for.
    #[must_use]
    pub fn clock_target(&self) -> (isize, f64) {
        *lock(&self.clock_target)
    }
}

impl<C, P, O, N> Engine<'_, C, P, O, N>
where
    C: FrameSource,
    P: Paint,
    O: FnMut(ClockKind) -> Result<C, ClockError>,
    N: Fn() -> f64,
{
    /// Hands a pending palette to the painter and forgets which page bitmaps were rastered, so
    /// the next raster pass redraws them all. True when a palette was pending.
    fn apply_palette(&mut self) -> bool {
        let Some(palette) = self.shared.take_palette() else {
            return false;
        };
        self.paint.set_palette(palette);
        self.warm = Default::default();
        self.presented = None;
        true
    }

    /// Applies a pending palette and relayout; `repaint` paints the settled, unsuppressed
    /// island once at the new layout (the parked loop has no frame of its own to show it). A
    /// failed rebuild falls back to WARP; a failed fallback ends the thread.
    pub(super) fn apply_relayout(&mut self, repaint: bool) -> Result<(), String> {
        let recolored = self.apply_palette();
        let Some(next) = self.shared.take_relayout() else {
            if recolored && repaint && !self.scene.suppression.any() && self.is_settled() {
                // The bitmaps are stale: raster the open pages again, then present once.
                self.prewarm_open(Reason::Rest);
                self.paint_frame()?;
            }
            return Ok(());
        };
        self.layout = next.layout;
        self.warm = Default::default();
        self.presented = None;
        if let Err(error) = self.paint.relayout(next.layout) {
            self.paint.recover(true).map_err(|warp| {
                format!("relayout failed: {error}; WARP fallback failed: {warp}")
            })?;
        }
        for line in self.paint.take_notes() {
            (self.sinks.on_note)(line);
        }
        let clock = match next.order {
            Some(order) => {
                self.hz = next.hz;
                self.period = 1.0 / next.hz;
                self.timeline = VsyncTimeline::new(next.hz);
                self.clocks.replace_order(order);
                "reopened"
            }
            None => "kept",
        };
        let s = next.layout.surface;
        (self.sinks.on_note)(format!(
            "render relayout: {}x{} scale={:.2} hz={:.2} clock={clock}",
            s.width, s.height, next.layout.scale, self.hz
        ));
        if repaint && !self.scene.suppression.any() && self.is_settled() {
            self.paint_frame()?;
        }
        Ok(())
    }
}
