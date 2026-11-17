//! The OSD side of the feed: volume and brightness changes go to a sink.

use super::{Feed, is_open};
use crate::osd::OsdSink;
use plith_core::osd::{OsdReading, changes, reading};
use plith_core::pages::PageId;
use plith_core::scene::Scene;

/// The OSD state of a feed: the sink and the reading it last saw.
#[derive(Default)]
pub struct OsdFeed {
    sink: Option<OsdSink>,
    seen: Option<OsdReading>,
}

impl Feed {
    /// Hands volume and brightness changes to `sink` from now on. The next drain takes the
    /// first reading without showing anything.
    pub fn set_osd(&mut self, sink: OsdSink) {
        self.osd.sink = Some(sink);
        self.osd.seen = None;
    }

    /// Compares the facts with the last OSD reading and feeds the sink. Without a sink, or on a
    /// wake that reported no change after the baseline, nothing is computed. The page that shows
    /// a meter keeps that meter quiet.
    pub(super) fn feed_osd(&mut self, scene: &Scene, changed: bool) {
        let Some(sink) = self.osd.sink.as_mut() else {
            return;
        };
        if !changed && self.osd.seen.is_some() {
            return;
        }
        let next = reading(&self.facts.audio, &self.facts.system);
        let open = if is_open(scene) {
            PageId::from_index(scene.page)
        } else {
            None
        };
        if let Some(prev) = &self.osd.seen {
            for level in changes(prev, &next, open) {
                sink(level);
            }
        }
        self.osd.seen = Some(next);
    }
}
