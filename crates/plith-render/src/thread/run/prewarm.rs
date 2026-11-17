//! Page bitmaps warmed outside the measured frames: the pages an aimed motion shows before its
//! first clock wait, the other pages once it settles, changed pages at rest and between ambient
//! ticks. A collapsed or hidden island rasters nothing. Every prewarm that rastered writes one
//! `prewarm:` line, so its cost stays visible next to the `motion` lines.

use super::super::Paint;
use super::engine::{Engine, opacity_f32};
use crate::clock::{ClockError, FrameSource};
use crate::paint::PaintFrame;
use plith_core::clock_choice::ClockKind;
use plith_core::pages::PAGE_COUNT;
use std::sync::Arc;

/// Why the pages were warmed, as the log line names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Reason {
    /// Before a motion's first clock wait.
    Aim,
    /// At rest: after a motion settled, or new content on a still island.
    Rest,
    /// New content between ambient ticks.
    Ambient,
}

impl Reason {
    fn as_str(self) -> &'static str {
        match self {
            Self::Aim => "aim",
            Self::Rest => "rest",
            Self::Ambient => "ambient",
        }
    }
}

impl<C, P, O, N> Engine<'_, C, P, O, N>
where
    C: FrameSource,
    P: Paint,
    O: FnMut(ClockKind) -> Result<C, ClockError>,
    N: Fn() -> f64,
{
    /// True when the island shows its pages: expanded and not suppressed.
    pub(super) fn is_open(&self) -> bool {
        self.scene.expanded && !self.scene.suppression.any()
    }

    /// The pages among `among` whose bitmap was not rastered from the current list.
    fn stale(&self, among: &[usize]) -> Vec<usize> {
        among
            .iter()
            .copied()
            .filter(|&page| {
                let (Some(list), Some(warm)) =
                    (self.page_content.pages.get(page), self.warm.get(page))
                else {
                    return false;
                };
                !warm.as_ref().is_some_and(|w| Arc::ptr_eq(w, list))
            })
            .collect()
    }

    /// Rasters the stale pages among `pages` and logs the cost when something was rastered.
    pub(super) fn prewarm(&mut self, pages: &[usize], reason: Reason) {
        let stale = self.stale(pages);
        if stale.is_empty() {
            return;
        }
        let ms = self.paint.prewarm(&self.page_content, &stale);
        for &page in &stale {
            self.warm[page] = Some(Arc::clone(&self.page_content.pages[page]));
        }
        if ms > 0.0 {
            let list: Vec<String> = stale.iter().map(usize::to_string).collect();
            (self.sinks.on_note)(format!(
                "prewarm: pages {} {ms:.2} ms ({})",
                list.join(","),
                reason.as_str()
            ));
        }
    }

    /// Warms every stale page while the island is open; nothing while collapsed or hidden.
    pub(super) fn prewarm_open(&mut self, reason: Reason) {
        if self.is_open() {
            let all: [usize; PAGE_COUNT] = std::array::from_fn(|page| page);
            self.prewarm(&all, reason);
        }
    }

    /// New content on a still island: warm the changed pages (when open), then present one
    /// frame only when a list the screen shows changed.
    pub(super) fn content_at_rest(&mut self) -> Result<(), String> {
        self.prewarm_open(Reason::Rest);
        if self.shown_changed() {
            self.paint_frame()?;
        }
        Ok(())
    }

    /// True when a list the current state shows differs from the last presented frame's: a
    /// visible page layer (content showing), the pill (content not fully showing) or the
    /// artwork. True before the first present.
    fn shown_changed(&self) -> bool {
        let Some(old) = self.presented.as_ref() else {
            return true;
        };
        let new = &self.page_content;
        let opacity = opacity_f32(self.content.value());
        let pages = opacity > 0.0
            && self.pages.layers().iter().any(|layer| {
                match (old.pages.get(layer.page), new.pages.get(layer.page)) {
                    (Some(a), Some(b)) => !Arc::ptr_eq(a, b),
                    _ => false,
                }
            });
        let pill = opacity < 1.0 && !Arc::ptr_eq(&old.pill, &new.pill);
        let art = old.art.len() != new.art.len()
            || old
                .art
                .iter()
                .zip(&new.art)
                .any(|(a, b)| !Arc::ptr_eq(a, b));
        pages || pill || art
    }

    /// Records a presented frame: its content, and every page layer it rastered.
    pub(super) fn note_presented(&mut self, frame: &PaintFrame) {
        self.presented = Some(Arc::clone(&frame.content));
        if frame.content_opacity <= 0.0 {
            return;
        }
        for layer in frame.pages.iter() {
            let shown = opacity_f32(layer.opacity) * frame.content_opacity > 0.0;
            if let (true, Some(list)) = (shown, frame.content.pages.get(layer.page)) {
                self.warm[layer.page] = Some(Arc::clone(list));
            }
        }
    }
}
