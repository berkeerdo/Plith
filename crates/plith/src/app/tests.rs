//! The idle look wiring through the app: timers at rest, suppression and the page an expand
//! lands on.

use super::feed::{CLOCK_TIMER, Feed, Timers, wanted};
use super::idle_facts::Idle;
use super::state::App;
use crate::log::{LogFile, Stamp};
use crate::services::fixture::FixtureServices;
use plith_core::draw::Node;
use plith_core::lang::Lang;
use plith_core::launch::Fixture;
use plith_core::pages::PageId;
use plith_core::scene::{Scene, SceneMailbox, ShapeCell, Suppression};
use plith_core::settings::{IdleLook, OpenOn, Settings};
use plith_shell::ShellEvent;
use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

const CLICK: ShellEvent = ShellEvent::Click { x: 5.0, y: 296.0 };

#[derive(Clone, Default)]
struct FakeTimers(Rc<RefCell<BTreeSet<usize>>>);

impl Timers for FakeTimers {
    fn set(&self, id: usize, _ms: u32) {
        self.0.borrow_mut().insert(id);
    }

    fn kill(&self, id: usize) {
        self.0.borrow_mut().remove(&id);
    }
}

fn running(timers: &FakeTimers) -> Vec<usize> {
    timers.0.borrow().iter().copied().collect()
}

fn stamp(minute: u16) -> Stamp {
    Stamp {
        year: 2026,
        month: 11,
        day: 17,
        hour: 21,
        minute,
        second: 10,
        millis: 0,
    }
}

struct Rig {
    app: App,
    timers: FakeTimers,
    minute: Rc<Cell<u16>>,
}

/// A fed app resting in `look`, opening on `open_on`.
fn rig(name: &str, look: IdleLook, open_on: OpenOn) -> Rig {
    let dir = std::env::temp_dir().join(format!("plith-wiring-{name}-{}", std::process::id()));
    let log = LogFile::open_at(&dir.join("plith.log")).unwrap();
    let mailbox = Arc::new(SceneMailbox::new(Scene::default(), Arc::default()));
    let mut app = App::new(
        Arc::clone(&mailbox),
        Arc::new(ShapeCell::default()),
        Arc::new(log),
        Arc::new(AtomicBool::new(false)),
    );
    let timers = FakeTimers::default();
    let minute = Rc::new(Cell::new(47));
    let clock = {
        let minute = Rc::clone(&minute);
        Box::new(move || stamp(minute.get()))
    };
    let mut feed = Feed::new(
        Box::new(FixtureServices::new(Fixture::Banana)),
        Box::new(timers.clone()),
        clock,
        Lang::Tr,
        &mailbox,
    );
    let settings = Settings {
        look,
        open_on,
        ..Settings::default()
    };
    feed.set_idle(Idle::new(settings, Lang::En), &mailbox);
    app.attach_feed(feed);
    Rig {
        app,
        timers,
        minute,
    }
}

fn fullscreen() -> ShellEvent {
    ShellEvent::Suppression(Suppression {
        fullscreen: true,
        ..Suppression::default()
    })
}

fn pill_texts(app: &App) -> Vec<String> {
    app.mailbox
        .latest()
        .content
        .pill
        .nodes
        .iter()
        .filter_map(|n| match n {
            Node::Text { text, .. } => Some(text.to_string()),
            _ => None,
        })
        .collect()
}

#[test]
fn clock_timer_armed_collapsed_for_karot_not_for_plain() {
    for look in [IdleLook::Karot, IdleLook::Clock] {
        let rig = rig("armed", look, OpenOn::Welcome);
        assert_eq!(running(&rig.timers), vec![CLOCK_TIMER], "{look:?}");
    }
    for look in [
        IdleLook::Plain,
        IdleLook::Hidden,
        IdleLook::Playing,
        IdleLook::Seal,
    ] {
        let rig = rig("quiet", look, OpenOn::Welcome);
        assert!(running(&rig.timers).is_empty(), "{look:?}");
    }
    let collapsed = Scene::default();
    assert!(wanted(&collapsed, Some(IdleLook::Karot)).clock);
    assert!(!wanted(&collapsed, Some(IdleLook::Plain)).clock);
    assert!(!wanted(&collapsed, None).clock, "no idle state, no timer");
}

#[test]
fn no_clock_timer_while_suppressed() {
    let mut rig = rig("suppressed", IdleLook::Karot, OpenOn::Welcome);
    rig.app.on_event(fullscreen());
    assert!(running(&rig.timers).is_empty());
    // A tick queued before the kill does nothing while suppressed.
    rig.minute.set(48);
    let generation = rig.app.mailbox.latest().generation;
    rig.app.on_event(ShellEvent::Timer(CLOCK_TIMER));
    assert_eq!(rig.app.mailbox.latest().generation, generation);
    assert!(running(&rig.timers).is_empty());
    rig.app
        .on_event(ShellEvent::Suppression(Suppression::default()));
    assert_eq!(
        running(&rig.timers),
        vec![CLOCK_TIMER],
        "back after it ends"
    );
}

#[test]
fn unsuppress_draws_once() {
    let mut rig = rig("catch-up", IdleLook::Karot, OpenOn::Welcome);
    assert!(pill_texts(&rig.app).contains(&"21:47".to_string()));
    rig.app.on_event(fullscreen());
    rig.minute.set(50);
    assert!(
        pill_texts(&rig.app).contains(&"21:47".to_string()),
        "nothing redraws while suppressed"
    );
    rig.app
        .on_event(ShellEvent::Suppression(Suppression::default()));
    assert!(pill_texts(&rig.app).contains(&"21:50".to_string()));
    let generation = rig.app.mailbox.latest().generation;
    rig.app.on_event(ShellEvent::Wake);
    rig.app
        .on_event(ShellEvent::Suppression(Suppression::default()));
    assert_eq!(
        rig.app.mailbox.latest().generation,
        generation,
        "the catch-up is one draw"
    );
}

#[test]
fn expand_lands_on_welcome_by_default_and_last_page_when_set() {
    let mut first = rig("open-welcome", IdleLook::Karot, OpenOn::Welcome);
    let _ = first.app.mailbox.update(|scene| Scene {
        page: PageId::Media.index(),
        ..scene
    });
    first.app.on_event(CLICK);
    let scene = first.app.mailbox.latest().scene;
    assert!(scene.expanded);
    assert_eq!(scene.page, PageId::Welcome.index());

    let mut last = rig("open-last", IdleLook::Karot, OpenOn::LastPage);
    let _ = last.app.mailbox.update(|scene| Scene {
        page: PageId::Media.index(),
        ..scene
    });
    last.app.on_event(CLICK);
    assert_eq!(
        last.app.mailbox.latest().scene.page,
        PageId::Media.index(),
        "the last page stays"
    );
}

#[test]
fn last_page_never_records_the_quick_page() {
    let mut rig = rig("open-last-quick", IdleLook::Karot, OpenOn::LastPage);
    let _ = rig.app.mailbox.update(|scene| Scene {
        page: PageId::Quick.index(),
        ..scene
    });
    rig.app.on_event(CLICK);
    assert_eq!(
        rig.app.mailbox.latest().scene.page,
        PageId::Welcome.index(),
        "the quick page is not a last page"
    );
}

fn collapse_by_demo(app: &mut App) {
    let (steps, queue) = std::sync::mpsc::channel();
    app.attach_demo(queue);
    steps.send(plith_core::demo::DemoStep::Collapse).unwrap();
    app.on_event(ShellEvent::Wake);
}

#[test]
fn collapsing_ends_the_probe_so_leave_rebuilds_nothing() {
    let mut rig = rig("probe-collapse", IdleLook::Plain, OpenOn::Welcome);
    rig.app.on_event(CLICK);
    rig.app.on_event(ShellEvent::Move { x: 100.0, y: 150.0 });
    assert!(rig.app.feed.as_ref().unwrap().probe().is_some());
    collapse_by_demo(&mut rig.app);
    assert!(!rig.app.mailbox.latest().scene.expanded);
    assert_eq!(rig.app.feed.as_ref().unwrap().probe(), None);
    let generation = rig.app.mailbox.latest().generation;
    rig.app.on_event(ShellEvent::Leave);
    assert_eq!(rig.app.mailbox.latest().generation, generation);
}

#[test]
fn the_hidden_look_publishes_the_hidden_rest_shape() {
    let rig = rig("hidden", IdleLook::Hidden, OpenOn::Welcome);
    assert_eq!(
        rig.app.mailbox.latest().scene.rest,
        plith_core::shapes::REST_HIDDEN
    );
    let other = self::rig("shown", IdleLook::Karot, OpenOn::Welcome);
    assert_eq!(
        other.app.mailbox.latest().scene.rest,
        plith_core::shapes::REST
    );
}

#[test]
fn a_collapsed_minute_tick_moves_the_pill_and_rearms() {
    let mut rig = rig("minute", IdleLook::Clock, OpenOn::Welcome);
    assert!(pill_texts(&rig.app).contains(&"21:47".to_string()));
    rig.minute.set(48);
    rig.app.on_event(ShellEvent::Timer(CLOCK_TIMER));
    assert!(pill_texts(&rig.app).contains(&"21:48".to_string()));
    assert_eq!(running(&rig.timers), vec![CLOCK_TIMER], "re-armed");
}

#[test]
fn a_plain_look_wakes_nothing_while_collapsed() {
    let mut rig = rig("plain-wake", IdleLook::Plain, OpenOn::Welcome);
    rig.minute.set(55);
    let generation = rig.app.mailbox.latest().generation;
    rig.app.on_event(ShellEvent::Wake);
    rig.app.on_event(ShellEvent::Timer(CLOCK_TIMER));
    assert_eq!(rig.app.mailbox.latest().generation, generation);
    assert!(running(&rig.timers).is_empty());
}

#[test]
fn the_welcome_page_follows_its_facts_when_open() {
    let mut rig = rig("welcome-live", IdleLook::Plain, OpenOn::Welcome);
    rig.app.on_event(CLICK);
    let before = rig.app.mailbox.latest().content.pages[PageId::Welcome.index()].clone();
    rig.minute.set(52);
    rig.app.on_event(ShellEvent::Timer(CLOCK_TIMER));
    let after = rig.app.mailbox.latest().content.pages[PageId::Welcome.index()].clone();
    assert_ne!(before, after, "the hero clock moved");
}

#[test]
fn exit_saves_the_play_log() {
    let dir = std::env::temp_dir().join(format!("plith-wiring-exit-{}", std::process::id()));
    let log = LogFile::open_at(&dir.join("plith.log")).unwrap();
    let mailbox = Arc::new(SceneMailbox::new(Scene::default(), Arc::default()));
    let mut feed = Feed::new(
        Box::new(FixtureServices::new(Fixture::Banana)),
        Box::new(FakeTimers::default()),
        Box::new(|| stamp(47)),
        Lang::Tr,
        &mailbox,
    );
    let saves = Rc::new(Cell::new(0));
    let seen = Rc::clone(&saves);
    let idle = Idle::new(Settings::default(), Lang::En)
        .with_saver(Box::new(move |_| seen.set(seen.get() + 1)));
    feed.set_idle(idle, &mailbox);
    assert_eq!(saves.get(), 0, "nothing is saved on attach");
    feed.stop();
    assert_eq!(saves.get(), 1);
    drop(log);
}
