//! Dragging a ruler on the open island: the press picks the ruler, every drag point shows the
//! value and (volume, brightness) writes it at a throttled pace, and the release writes the
//! final value. A seek is written only on the release.

use super::super::state::App;
use super::dispatch;
use plith_core::input::drag::{DRAG_WRITE_GAP_S, RulerDrag, RulerKind, Throttle, begin_drag};
use plith_core::input::page_on_screen;
use plith_core::page::PageLayers;
use plith_core::pages::Action;

/// A drag that began on a ruler of the open island.
pub struct ActiveDrag {
    ruler: RulerDrag,
    /// The page's x offset on the island when the drag began.
    offset: f64,
    throttle: Throttle,
}

/// The value an action carries on its ruler: a percent, or a fraction for seek.
fn value_of(action: &Action) -> Option<f64> {
    match action {
        Action::SetVolume(percent) | Action::SetBrightness(percent) => Some(f64::from(*percent)),
        Action::Seek(fraction) => Some(*fraction),
        _ => None,
    }
}

impl App {
    /// A counted press at `(x, y)` island DIP. Only an open island over a ruler starts a drag;
    /// a press elsewhere does nothing (its click, if any, comes on the release).
    pub(in crate::app) fn on_press(&mut self, x: f64, y: f64) {
        self.drag = None;
        if !self.mailbox.latest().scene.expanded {
            return;
        }
        let layers = self
            .shapes
            .latest()
            .map_or_else(PageLayers::empty, |frame| frame.pages);
        let (Some(layer), Some(feed)) = (page_on_screen(&layers), self.feed.as_ref()) else {
            return;
        };
        let Some(page) = feed.targets().get(layer.page) else {
            return;
        };
        self.drag = begin_drag(page, x - layer.offset, y).map(|ruler| ActiveDrag {
            ruler,
            offset: layer.offset,
            throttle: Throttle::new(DRAG_WRITE_GAP_S),
        });
    }

    /// The pointer moved to `(x, y)` with a press held: shows the value and writes it when the
    /// throttle allows.
    pub(in crate::app) fn on_drag(&mut self, x: f64, _y: f64) {
        let now = (self.now)();
        let (Some(drag), Some(feed)) = (self.drag.as_mut(), self.feed.as_mut()) else {
            return;
        };
        let action = drag.ruler.value_at(x - drag.offset);
        let Some(value) = value_of(&action) else {
            return;
        };
        feed.set_override(drag.ruler.kind, value, now, &self.mailbox);
        if drag.ruler.kind != RulerKind::Seek && drag.throttle.due(now) {
            dispatch(feed, &self.mailbox, action);
        }
    }

    /// The press ended after a drag at `(x, y)`: writes the final value (the only seek write)
    /// and ends the drag. The shown value stays until the reading catches up.
    pub(in crate::app) fn on_release(&mut self, x: f64, _y: f64) {
        let now = (self.now)();
        let (Some(drag), Some(feed)) = (self.drag.take(), self.feed.as_mut()) else {
            return;
        };
        let action = drag.ruler.value_at(x - drag.offset);
        if let Some(value) = value_of(&action) {
            feed.set_override(drag.ruler.kind, value, now, &self.mailbox);
        }
        dispatch(feed, &self.mailbox, action);
    }
}
