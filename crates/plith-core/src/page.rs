//! Page turns as springs, so an interrupted turn keeps its velocity.

use crate::spring::Spring;
use crate::tuning::{OPACITY, PAGE_OFFSET};

/// Distance in DIP a page slides while it enters or leaves.
pub const PAGE_SLIDE: f64 = 20.0;

/// One visible page with its current slide offset and opacity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageLayer {
    /// Page index.
    pub page: usize,
    /// Horizontal slide offset in DIP.
    pub offset: f64,
    /// Opacity from 0 to 1.
    pub opacity: f64,
}

#[derive(Debug, Clone)]
struct Layer {
    page: usize,
    offset: Spring,
    opacity: Spring,
}

impl Layer {
    fn resting(page: usize) -> Self {
        Self {
            page,
            offset: Spring::new(0.0, PAGE_OFFSET),
            opacity: Spring::new(1.0, OPACITY).with_eps(0.002, 0.01),
        }
    }

    fn entering(page: usize, dir: f64) -> Self {
        let mut layer = Self {
            page,
            offset: Spring::new(dir * PAGE_SLIDE, PAGE_OFFSET),
            opacity: Spring::new(0.0, OPACITY).with_eps(0.002, 0.01),
        };
        layer.show();
        layer
    }

    fn show(&mut self) {
        self.offset.retarget(0.0, PAGE_OFFSET);
        self.opacity.retarget(1.0, OPACITY);
    }

    fn hide(&mut self, dir: f64) {
        self.offset.retarget(-dir * PAGE_SLIDE, PAGE_OFFSET);
        self.opacity.retarget(0.0, OPACITY);
    }

    fn step(&mut self, dt: f64) -> bool {
        let offset_settled = self.offset.step(dt);
        let opacity_settled = self.opacity.step(dt);
        offset_settled && opacity_settled
    }

    fn snapshot(&self) -> PageLayer {
        PageLayer {
            page: self.page,
            offset: self.offset.value(),
            opacity: self.opacity.value(),
        }
    }
}

/// The incoming page and, while a turn is in flight, the outgoing one.
#[derive(Debug, Clone)]
pub struct PageTurn {
    incoming: Layer,
    outgoing: Option<Layer>,
}

impl PageTurn {
    /// Starts at rest on `page`.
    #[must_use]
    pub fn new(page: usize) -> Self {
        Self {
            incoming: Layer::resting(page),
            outgoing: None,
        }
    }

    /// Turns to `page`; a turn to the current page does nothing.
    pub fn turn_to(&mut self, page: usize) {
        if page == self.incoming.page {
            return;
        }
        let dir = if page > self.incoming.page { 1.0 } else { -1.0 };
        let back_to_outgoing = self.outgoing.as_ref().is_some_and(|o| o.page == page);
        if back_to_outgoing && let Some(mut returning) = self.outgoing.take() {
            returning.show();
            self.incoming.hide(dir);
            self.outgoing = Some(std::mem::replace(&mut self.incoming, returning));
            return;
        }
        let mut leaving = std::mem::replace(&mut self.incoming, Layer::entering(page, dir));
        leaving.hide(dir);
        self.outgoing = Some(leaving);
    }

    /// Advances by `dt` seconds; true when one layer remains and it is settled.
    pub fn step(&mut self, dt: f64) -> bool {
        let incoming_settled = self.incoming.step(dt);
        if let Some(out) = self.outgoing.as_mut() {
            out.step(dt);
            if out.opacity.is_settled() && out.opacity.target() == 0.0 {
                self.outgoing = None;
            }
        }
        self.outgoing.is_none() && incoming_settled
    }

    /// Visible layers, outgoing first.
    #[must_use]
    pub fn layers(&self) -> Vec<PageLayer> {
        self.outgoing
            .iter()
            .chain(std::iter::once(&self.incoming))
            .map(Layer::snapshot)
            .collect()
    }

    /// The page being turned to (or resting on).
    #[must_use]
    pub fn current(&self) -> usize {
        self.incoming.page
    }
}

#[cfg(test)]
mod tests;
