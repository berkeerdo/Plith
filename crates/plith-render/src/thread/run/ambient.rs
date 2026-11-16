//! The ambient tick: while the settled island shows a page with live nodes, the loop waits on
//! the frame clock every period and paints every `ambient_divisor(hz)`-th tick (each painted
//! frame samples fresh values). It writes no motion line, and one `ambient` line when it ends.

use super::super::Paint;
use super::engine::{Engine, Taken};
use super::prewarm::Reason;
use crate::clock::{ClockError, FrameSource};
use plith_core::ambient::{AmbientStats, ambient_divisor};
use plith_core::clock_choice::ClockKind;
use plith_core::scene::SceneMailbox;

/// Why an ambient run ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AmbientEnd {
    /// Nothing live to show any more (collapse without motion, suppression, a page or content
    /// without live nodes): park on the mailbox.
    Parked,
    /// The scene changed and the springs are aimed: run the motion.
    Moving,
    /// A stop was requested.
    Stopped,
}

impl<C, P, O, N> Engine<'_, C, P, O, N>
where
    C: FrameSource,
    P: Paint,
    O: FnMut(ClockKind) -> Result<C, ClockError>,
    N: Fn() -> f64,
{
    /// Ticks ambient frames until the scene moves, nothing live is left or a stop is requested,
    /// then reports the run through `on_ambient` (when it painted a frame).
    pub(super) fn ambient(&mut self, mailbox: &SceneMailbox) -> Result<AmbientEnd, String> {
        self.restart_clock();
        let divisor = ambient_divisor(self.hz);
        let mut stats = AmbientStats::new(self.scene.page, self.hz);
        let mut phase = 0_u32;
        let end = loop {
            if self.shared.stopping() {
                break AmbientEnd::Stopped;
            }
            let Some(raw) = self.wait_clock()? else {
                continue;
            };
            let Some(tick) = self.timeline.lock(raw) else {
                continue;
            };
            let (anim_t, _) = self.stamper.stamp(tick);
            if let Some(end) = self.ambient_news(mailbox)? {
                break end;
            }
            phase = phase.saturating_add(tick.periods);
            if phase < divisor {
                continue;
            }
            phase = 0;
            if let Some(times) = self.paint_frame()? {
                stats.record_frame(anim_t, times.draw_ms);
            }
            self.wait_ms = 0.0;
        };
        if stats.frames() > 0 {
            (self.sinks.on_ambient)(stats.finish());
        }
        Ok(end)
    }

    /// Takes a newer mailbox generation, if any. `Some` ends the run; new content (or a scene
    /// that needs no motion) keeps it going while a live node remains, and otherwise paints the
    /// new state once before parking.
    fn ambient_news(&mut self, mailbox: &SceneMailbox) -> Result<Option<AmbientEnd>, String> {
        let snapshot = mailbox.latest();
        if snapshot.generation == self.seen {
            return Ok(None);
        }
        match self.take(snapshot) {
            Taken::Snapped => return Ok(Some(AmbientEnd::Parked)),
            Taken::Aimed if !self.is_settled() => return Ok(Some(AmbientEnd::Moving)),
            Taken::Aimed => {
                self.stats = None;
                self.aim_pages.clear();
            }
            // Between ticks: the next painted tick then finds the bitmaps fresh.
            Taken::ContentOnly => self.prewarm_open(Reason::Ambient),
        }
        if self.wants_ambient() {
            return Ok(None);
        }
        self.paint_frame()?;
        Ok(Some(AmbientEnd::Parked))
    }
}
