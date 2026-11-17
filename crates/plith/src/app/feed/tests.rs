use super::*;
use crate::app::idle_facts::Idle;
use crate::services::fixture::FixtureServices;
use crate::services::{Changes, LevelSource};
use plith_core::launch::Fixture;
use plith_core::osd::{OsdLevel, OsdMeter};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

/// Services whose facts the test sets; every write is ignored.
struct FakeServices {
    facts: Rc<RefCell<Facts>>,
    /// What the next drain reports.
    drained: Rc<Cell<Changes>>,
}

impl Services for FakeServices {
    fn drain(&mut self) -> Changes {
        self.drained.get()
    }

    fn facts(&self) -> Facts {
        self.facts.borrow().clone()
    }

    fn level_source(&self) -> LevelSource {
        Box::new(|| None)
    }

    fn set_default_endpoint(&mut self, _id: &str) {}
    fn toggle_route(&mut self, _strip: usize, _bus: usize) {}
    fn toggle_mic(&mut self) {}
    fn set_volume(&mut self, _percent: u8) {}
    fn set_brightness(&mut self, _percent: u8) {}
    fn play_pause(&mut self) {}
    fn next(&mut self) {}
    fn previous(&mut self) {}
    fn seek(&mut self, _position: Duration) {}
}

struct NoTimers;

impl Timers for NoTimers {
    fn set(&self, _id: usize, _ms: u32) {}
    fn kill(&self, _id: usize) {}
}

fn stamp() -> Stamp {
    Stamp {
        year: 2026,
        month: 11,
        day: 17,
        hour: 9,
        minute: 5,
        second: 0,
        millis: 0,
    }
}

struct Rig {
    feed: Feed,
    mailbox: SceneMailbox,
    facts: Rc<RefCell<Facts>>,
    drained: Rc<Cell<Changes>>,
    shown: Rc<RefCell<Vec<OsdLevel>>>,
}

fn rig() -> Rig {
    let mut base = FixtureServices::new(Fixture::Bare).facts();
    base.audio.attached = Some((0.5, false));
    base.system.brightness = None;
    let facts = Rc::new(RefCell::new(base));
    let drained = Rc::new(Cell::new(Changes::AUDIO));
    let mailbox = SceneMailbox::new(Scene::default(), Arc::default());
    let mut feed = Feed::new(
        Box::new(FakeServices {
            facts: Rc::clone(&facts),
            drained: Rc::clone(&drained),
        }),
        Box::new(NoTimers),
        Box::new(stamp),
        &mailbox,
    );
    let shown = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&shown);
    feed.set_osd(Box::new(move |level| sink.borrow_mut().push(level)));
    Rig {
        feed,
        mailbox,
        facts,
        drained,
        shown,
    }
}

fn set_volume(rig: &Rig, scalar: f32) {
    rig.facts.borrow_mut().audio.attached = Some((scalar, false));
}

fn open_page(rig: &Rig, page: usize) {
    let _ = rig.mailbox.update(|scene| Scene {
        expanded: true,
        page,
        ..scene
    });
}

#[test]
fn no_osd_on_first_drain() {
    let mut rig = rig();
    rig.feed.on_wake(&rig.mailbox);
    assert!(rig.shown.borrow().is_empty());
}

#[test]
fn volume_change_reaches_the_sink() {
    let mut rig = rig();
    rig.feed.on_wake(&rig.mailbox);
    set_volume(&rig, 0.8);
    rig.feed.on_wake(&rig.mailbox);
    let shown = rig.shown.borrow();
    assert_eq!(shown.len(), 1);
    assert_eq!(shown[0].meter, OsdMeter::Volume);
    assert_eq!(shown[0].percent, 80);
}

#[test]
fn open_audio_page_keeps_volume_quiet() {
    let mut rig = rig();
    rig.feed.on_wake(&rig.mailbox);
    open_page(&rig, 0);
    set_volume(&rig, 0.8);
    rig.feed.on_wake(&rig.mailbox);
    assert!(rig.shown.borrow().is_empty());
}

#[test]
fn a_wake_without_changes_takes_no_reading() {
    let mut rig = rig();
    rig.feed.on_wake(&rig.mailbox);
    // The facts moved but no change is reported: nothing is read, so nothing is shown.
    rig.drained.set(Changes::NONE);
    set_volume(&rig, 0.8);
    rig.feed.on_wake(&rig.mailbox);
    assert!(rig.shown.borrow().is_empty());
    // The next reported change still compares against the baseline of the first wake.
    rig.drained.set(Changes::AUDIO);
    rig.feed.on_wake(&rig.mailbox);
    assert_eq!(rig.shown.borrow().len(), 1);
}

#[test]
fn the_baseline_is_taken_on_a_wake_without_changes() {
    let mut rig = rig();
    rig.drained.set(Changes::NONE);
    rig.feed.on_wake(&rig.mailbox);
    rig.drained.set(Changes::AUDIO);
    set_volume(&rig, 0.8);
    rig.feed.on_wake(&rig.mailbox);
    assert_eq!(rig.shown.borrow().len(), 1);
}

#[test]
fn another_open_page_still_shows_volume() {
    let mut rig = rig();
    rig.feed.on_wake(&rig.mailbox);
    open_page(&rig, 1);
    set_volume(&rig, 0.8);
    rig.feed.on_wake(&rig.mailbox);
    assert_eq!(rig.shown.borrow().len(), 1);
}

fn playing_pill_texts(mailbox: &SceneMailbox) -> Vec<String> {
    mailbox
        .latest()
        .content
        .pill
        .nodes
        .iter()
        .filter_map(|n| match n {
            plith_core::draw::Node::Text { text, .. } => Some(text.to_string()),
            _ => None,
        })
        .collect()
}

fn set_suppressed(rig: &Rig, fullscreen: bool) {
    let _ = rig.mailbox.update(|scene| Scene {
        suppression: plith_core::scene::Suppression {
            fullscreen,
            ..Default::default()
        },
        ..scene
    });
}

#[test]
fn a_media_wake_while_suppressed_publishes_nothing_and_unsuppress_publishes_once() {
    use plith_core::lang::Lang;
    use plith_core::settings::{IdleLook, Settings};
    let mut rig = rig();
    {
        let mut facts = rig.facts.borrow_mut();
        facts.media.has_session = true;
        facts.media.playing = true;
        facts.media.title = "First".to_string();
        facts.media.artist = "Band".to_string();
    }
    let settings = Settings {
        look: IdleLook::Playing,
        ..Settings::default()
    };
    rig.feed
        .set_idle(Idle::new(settings, Lang::En), &rig.mailbox);
    rig.feed.on_wake(&rig.mailbox);
    set_suppressed(&rig, true);
    rig.feed.on_wake(&rig.mailbox);
    let generation = rig.mailbox.latest().generation;
    let before = playing_pill_texts(&rig.mailbox);

    rig.facts.borrow_mut().media.title = "Second".to_string();
    rig.drained.set(Changes::MEDIA);
    rig.feed.on_wake(&rig.mailbox);
    assert_eq!(rig.mailbox.latest().generation, generation, "no publish");
    assert_eq!(playing_pill_texts(&rig.mailbox), before);

    set_suppressed(&rig, false);
    let scene_generation = rig.mailbox.latest().generation;
    rig.drained.set(Changes::NONE);
    rig.feed.on_wake(&rig.mailbox);
    let after = playing_pill_texts(&rig.mailbox);
    assert!(after.iter().any(|t| t.contains("Second")), "{after:?}");
    let published = rig.mailbox.latest().generation - scene_generation;
    assert!(published >= 1);
    // Nothing more follows: the catch-up was taken once.
    let settled = rig.mailbox.latest().generation;
    rig.feed.on_wake(&rig.mailbox);
    assert_eq!(rig.mailbox.latest().generation, settled);
}
