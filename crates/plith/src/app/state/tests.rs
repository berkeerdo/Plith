use super::*;
use crate::app::feed::{
    ACTIVITY_TIMER, CLOCK_TIMER, COLLAPSE_TIMER, Feed, MEDIA_TIMER, Timers, clock_ms,
};
use crate::log::Stamp;
use crate::services::fixture::FixtureServices;
use plith_core::launch::Fixture;
use plith_core::page::{PageLayer, PageLayers};
use plith_core::pages::audio::PickMode;
use plith_core::pages::{Action, PageId};
use plith_core::scene::{ShapeCell, ShapeFrame, Suppression};
use plith_core::shapes::PAGES;
use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::rc::Rc;

#[derive(Clone, Default)]
struct FakeStop(Rc<Cell<u32>>);

impl Stopper for FakeStop {
    fn stop_and_join(&self) {
        self.0.set(self.0.get() + 1);
    }
}

fn app(name: &str) -> App {
    let dir = std::env::temp_dir().join(format!("plith-app-{name}-{}", std::process::id()));
    let log = LogFile::open_at(&dir.join("plith.log")).unwrap();
    let mailbox = Arc::new(SceneMailbox::new(Scene::default(), Arc::default()));
    App::new(
        mailbox,
        Arc::new(ShapeCell::default()),
        Arc::new(log),
        Arc::new(AtomicBool::new(false)),
    )
}

#[test]
fn closing_before_render_attached_is_safe() {
    let mut app = app("early");
    app.on_event(ShellEvent::Closing);
    assert!(app.closing.load(Ordering::Acquire));
}

#[test]
fn closing_stops_the_attached_render() {
    let mut app = app("stop");
    let fake = FakeStop::default();
    app.attach_render(fake.clone());
    app.on_event(ShellEvent::Closing);
    assert_eq!(fake.0.get(), 1);
    assert!(app.closing.load(Ordering::Acquire));
}

#[test]
fn click_expands_and_empty_space_keeps_it_open() {
    let mut app = app("click");
    app.on_event(CLICK);
    assert!(app.mailbox.latest().scene.expanded);
    show(&app, PageId::Audio);
    app.on_event(CLICK);
    assert!(app.mailbox.latest().scene.expanded, "no target: stays open");
    assert_eq!(app.mailbox.latest().generation, 1);
}

#[test]
fn suppression_republishes() {
    let mut app = app("suppress");
    let suppression = Suppression {
        locked: true,
        ..Suppression::default()
    };
    app.on_event(ShellEvent::Suppression(suppression));
    assert_eq!(app.mailbox.latest().scene.suppression, suppression);
    assert_eq!(app.mailbox.latest().generation, 1);
}

#[test]
fn closing_is_set_before_the_stopper_runs() {
    struct SeesClosing(Arc<AtomicBool>, Rc<Cell<Option<bool>>>);
    impl Stopper for SeesClosing {
        fn stop_and_join(&self) {
            self.1.set(Some(self.0.load(Ordering::Acquire)));
        }
    }
    let mut app = app("order");
    let seen = Rc::new(Cell::new(None));
    app.attach_render(SeesClosing(Arc::clone(&app.closing), Rc::clone(&seen)));
    app.on_event(ShellEvent::Closing);
    assert_eq!(seen.get(), Some(true));
}

#[test]
fn wake_without_a_feed_changes_nothing() {
    let mut app = app("wake");
    app.on_event(ShellEvent::Wake);
    assert_eq!(app.mailbox.latest().generation, 0);
}

/// Records which timers run.
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

fn stamp(minute: u16) -> Stamp {
    Stamp {
        year: 2026,
        month: 11,
        day: 16,
        hour: 21,
        minute,
        second: 10,
        millis: 0,
    }
}

/// An app fed by `kind`, its timers and the minute its clock reads.
fn fed(name: &str, kind: Fixture) -> (App, FakeTimers, Rc<Cell<u16>>) {
    let (app, timers, minute, _) = clocked(name, kind);
    (app, timers, minute)
}

/// As [`fed`], plus the app's monotonic clock in seconds.
fn clocked(name: &str, kind: Fixture) -> (App, FakeTimers, Rc<Cell<u16>>, Rc<Cell<f64>>) {
    let seconds = Rc::new(Cell::new(0.0));
    let now = {
        let seconds = Rc::clone(&seconds);
        Box::new(move || seconds.get())
    };
    let mut app = app(name).with_clock(now);
    let timers = FakeTimers::default();
    let minute = Rc::new(Cell::new(47));
    let clock = {
        let minute = Rc::clone(&minute);
        Box::new(move || stamp(minute.get()))
    };
    let feed = Feed::new(
        Box::new(FixtureServices::new(kind)),
        Box::new(timers.clone()),
        clock,
        &app.mailbox,
    );
    app.attach_feed(feed);
    (app, timers, minute, seconds)
}

fn running(timers: &FakeTimers) -> Vec<usize> {
    timers.0.borrow().iter().copied().collect()
}

fn go_to(app: &App, page: PageId) {
    let _ = app.mailbox.update(|scene| Scene {
        page: page.index(),
        ..scene
    });
}

#[test]
fn attaching_the_feed_publishes_every_page() {
    let (app, timers, _) = fed("feed-first", Fixture::Banana);
    let latest = app.mailbox.latest();
    assert!(latest.generation > 0);
    assert!(latest.content.pages.iter().all(|p| !p.nodes.is_empty()));
    assert!(!latest.content.pill.nodes.is_empty());
    assert!(running(&timers).is_empty(), "collapsed: no timers");
}

#[test]
fn wake_with_no_change_publishes_nothing() {
    let (mut app, _, _) = fed("feed-wake", Fixture::Banana);
    let generation = app.mailbox.latest().generation;
    app.on_event(ShellEvent::Wake);
    app.on_event(ShellEvent::Wake);
    assert_eq!(app.mailbox.latest().generation, generation);
}

#[test]
fn wake_after_a_write_publishes_the_change() {
    let (mut app, _, _) = fed("feed-write", Fixture::Banana);
    app.on_event(CLICK);
    let before = app.mailbox.latest();
    app.feed.as_mut().unwrap().services().set_brightness(10);
    app.on_event(ShellEvent::Wake);
    let after = app.mailbox.latest();
    assert!(after.generation > before.generation);
    assert!(Arc::ptr_eq(
        &before.content.pages[0],
        &after.content.pages[0]
    ));
    assert_ne!(before.content.pages[3], after.content.pages[3]);
}

#[test]
fn activity_timer_only_on_audio_page() {
    let (mut app, timers, _, seconds) = clocked("feed-activity", Fixture::Banana);
    app.on_event(CLICK);
    assert_eq!(running(&timers), vec![ACTIVITY_TIMER, CLOCK_TIMER]);
    go_to(&app, PageId::Media);
    app.on_event(ShellEvent::Timer(ACTIVITY_TIMER));
    assert_eq!(running(&timers), vec![MEDIA_TIMER, CLOCK_TIMER]);
    go_to(&app, PageId::System);
    app.on_event(ShellEvent::Timer(MEDIA_TIMER));
    assert_eq!(running(&timers), vec![CLOCK_TIMER]);
    go_to(&app, PageId::Audio);
    app.on_event(ShellEvent::Timer(CLOCK_TIMER));
    assert_eq!(running(&timers), vec![ACTIVITY_TIMER, CLOCK_TIMER]);
    collapse(&mut app, &seconds);
    assert!(running(&timers).is_empty(), "collapsed: no timers");
}

#[test]
fn suppression_stops_the_timers() {
    let (mut app, timers, _) = fed("feed-suppress", Fixture::Banana);
    app.on_event(CLICK);
    assert!(!running(&timers).is_empty());
    app.on_event(ShellEvent::Suppression(Suppression {
        fullscreen: true,
        ..Suppression::default()
    }));
    assert!(running(&timers).is_empty());
}

#[test]
fn no_media_timer_while_paused() {
    let (mut app, timers, _) = fed("feed-paused", Fixture::Banana);
    app.feed.as_mut().unwrap().services().play_pause();
    app.on_event(ShellEvent::Wake);
    go_to(&app, PageId::Media);
    app.on_event(CLICK);
    assert_eq!(running(&timers), vec![CLOCK_TIMER]);
}

#[test]
fn activity_tick_lights_the_routes() {
    let (mut app, _, _) = fed("feed-lit", Fixture::Banana);
    let before = app.mailbox.latest();
    app.on_event(CLICK);
    let after = app.mailbox.latest();
    assert!(after.generation > before.generation);
    assert_ne!(
        before.content.pages[0], after.content.pages[0],
        "opening on the audio page reads the levels at once"
    );
    let generation = after.generation;
    app.on_event(ShellEvent::Timer(ACTIVITY_TIMER));
    assert_eq!(
        app.mailbox.latest().generation,
        generation,
        "the same signal publishes nothing"
    );
}

#[test]
fn the_clock_tick_rewrites_the_tab_bars() {
    let (mut app, timers, minute) = fed("feed-clock", Fixture::Banana);
    app.on_event(CLICK);
    let generation = app.mailbox.latest().generation;
    app.on_event(ShellEvent::Timer(CLOCK_TIMER));
    assert_eq!(app.mailbox.latest().generation, generation, "same minute");
    minute.set(48);
    app.on_event(ShellEvent::Timer(CLOCK_TIMER));
    let latest = app.mailbox.latest();
    assert!(latest.generation > generation);
    let has = |text: &str| {
        latest.content.pages[2]
            .nodes
            .iter()
            .any(|n| matches!(n, plith_core::draw::Node::Text { text: t, .. } if &**t == text))
    };
    assert!(has("21:48"));
    assert!(running(&timers).contains(&CLOCK_TIMER), "re-armed");
}

#[test]
fn opening_after_a_minute_refreshes_the_clock() {
    let (mut app, _, minute) = fed("feed-open", Fixture::Banana);
    minute.set(50);
    app.on_event(CLICK);
    let latest = app.mailbox.latest();
    let has = latest.content.pages[0]
        .nodes
        .iter()
        .any(|n| matches!(n, plith_core::draw::Node::Text { text, .. } if &**text == "21:50"));
    assert!(has);
}

#[test]
fn a_tick_after_collapse_publishes_nothing() {
    let (mut app, timers, _, seconds) = clocked("feed-stale", Fixture::Banana);
    app.on_event(CLICK);
    collapse(&mut app, &seconds);
    let generation = app.mailbox.latest().generation;
    for id in [ACTIVITY_TIMER, MEDIA_TIMER, CLOCK_TIMER] {
        app.on_event(ShellEvent::Timer(id));
    }
    assert_eq!(app.mailbox.latest().generation, generation);
    assert!(running(&timers).is_empty());
}

#[test]
fn a_scene_from_another_publisher_reaches_the_feed_on_wake() {
    let (mut app, timers, _) = fed("feed-demo", Fixture::Banana);
    let _ = app.mailbox.update(|scene| Scene {
        expanded: true,
        ..scene
    });
    app.on_event(ShellEvent::Wake);
    assert_eq!(running(&timers), vec![ACTIVITY_TIMER, CLOCK_TIMER]);
    go_to(&app, PageId::System);
    app.on_event(ShellEvent::Wake);
    assert_eq!(running(&timers), vec![CLOCK_TIMER]);
    let _ = app.mailbox.update(|scene| Scene {
        expanded: false,
        ..scene
    });
    app.on_event(ShellEvent::Wake);
    assert!(running(&timers).is_empty());
}

#[test]
fn collapsed_changes_publish_only_the_pill_until_opening() {
    let (mut app, _, _) = fed("feed-quiet", Fixture::Banana);
    let first = app.mailbox.latest();
    app.feed.as_mut().unwrap().services().set_brightness(10);
    app.on_event(ShellEvent::Wake);
    app.feed.as_mut().unwrap().services().play_pause();
    app.on_event(ShellEvent::Wake);
    assert_eq!(
        app.mailbox.latest().generation,
        first.generation,
        "neither change touches the pill"
    );
    app.feed
        .as_mut()
        .unwrap()
        .services()
        .set_default_endpoint("r1280");
    app.on_event(ShellEvent::Wake);
    let collapsed = app.mailbox.latest();
    assert_eq!(collapsed.generation, first.generation + 1, "the pill only");
    assert_ne!(collapsed.content.pill, first.content.pill);
    for i in 0..4 {
        assert!(Arc::ptr_eq(
            &collapsed.content.pages[i],
            &first.content.pages[i]
        ));
    }
    app.on_event(CLICK);
    let open = app.mailbox.latest();
    assert_ne!(open.content.pages[1], first.content.pages[1]);
    assert_ne!(open.content.pages[3], first.content.pages[3]);
    let generation = open.generation;
    app.on_event(ShellEvent::Wake);
    assert_eq!(app.mailbox.latest().generation, generation, "flushed once");
}

#[test]
fn clock_ms_lands_after_the_minute() {
    let mut now = stamp(47);
    now.second = 59;
    now.millis = 900;
    assert_eq!(clock_ms(now), 100 + 20);
    now.second = 0;
    now.millis = 0;
    assert_eq!(clock_ms(now), 60_000 + 20);
}

/// A click on the drawn island where no page target is.
const CLICK: ShellEvent = ShellEvent::Click { x: 5.0, y: 296.0 };

/// Publishes a drawn frame that shows `page` at rest.
fn show(app: &App, page: PageId) {
    let shape = PAGES[page.index()];
    app.shapes.publish(ShapeFrame {
        shape,
        previous: shape,
        generation: 1,
        pages: PageLayers::of(&[PageLayer {
            page: page.index(),
            offset: 0.0,
            opacity: 1.0,
        }]),
    });
}

/// Leaves the island and lets the collapse delay run out.
fn collapse(app: &mut App, seconds: &Cell<f64>) {
    app.on_event(ShellEvent::Leave);
    seconds.set(seconds.get() + 0.5);
    app.on_event(ShellEvent::Timer(COLLAPSE_TIMER));
}

/// The centre of the first target on `page` whose action matches.
fn centre_of(app: &App, page: PageId, wanted: impl Fn(&Action) -> bool) -> (f64, f64) {
    let feed = app.feed.as_ref().expect("fed");
    let target = feed.targets()[page.index()]
        .iter()
        .find(|t| wanted(&t.action))
        .expect("target on the page");
    (
        target.rect.x + target.rect.w / 2.0,
        target.rect.y + target.rect.h / 2.0,
    )
}

/// Opens the island on `page` and clicks the first target matching `wanted`.
fn press(app: &mut App, page: PageId, wanted: impl Fn(&Action) -> bool) {
    if !app.mailbox.latest().scene.expanded {
        app.on_event(CLICK);
    }
    go_to(app, page);
    show(app, page);
    let (x, y) = centre_of(app, page, wanted);
    app.on_event(ShellEvent::Click { x, y });
}

fn facts(app: &mut App) -> crate::services::Facts {
    app.feed.as_mut().unwrap().services().facts(0.0)
}

fn pick(app: &App) -> PickMode {
    app.feed.as_ref().unwrap().pick()
}

#[test]
fn click_on_a_tab_turns_the_page() {
    let (mut app, timers, _) = fed("input-tab", Fixture::Banana);
    press(&mut app, PageId::Audio, |a| {
        *a == Action::Tab(PageId::System)
    });
    let scene = app.mailbox.latest().scene;
    assert!(scene.expanded);
    assert_eq!(scene.page, PageId::System.index());
    assert_eq!(running(&timers), vec![CLOCK_TIMER], "the feed saw the turn");
}

#[test]
fn pick_source_then_output_sets_the_route() {
    let (mut app, _, _) = fed("input-pick", Fixture::Banana);
    let route = |app: &mut App| {
        facts(app).audio.vm.unwrap().strips[0]
            .routes
            .get(2)
            .copied()
    };
    let before = route(&mut app);
    press(&mut app, PageId::Audio, |a| *a == Action::PickSource(0));
    assert_eq!(pick(&app), PickMode::Picked(0));
    let picked = app.mailbox.latest().content.pages[0].clone();
    press(&mut app, PageId::Audio, |a| *a == Action::PickSource(0));
    assert_eq!(pick(&app), PickMode::None, "the same source again cancels");
    assert_ne!(app.mailbox.latest().content.pages[0], picked, "rebuilt");
    press(&mut app, PageId::Audio, |a| *a == Action::PickSource(0));
    press(&mut app, PageId::Audio, |a| *a == Action::PickOutput(2));
    assert_eq!(pick(&app), PickMode::None, "a route ends the pick");
    assert_ne!(route(&mut app), before, "the fixture route flipped");
}

#[test]
fn an_output_without_a_pick_does_nothing() {
    let (mut app, _, _) = fed("input-nopick", Fixture::Banana);
    let before = facts(&mut app).audio;
    press(&mut app, PageId::Audio, |a| *a == Action::PickOutput(2));
    assert_eq!(facts(&mut app).audio, before);
}

#[test]
fn click_on_the_empty_diagram_cancels_the_pick() {
    let (mut app, _, _) = fed("input-cancel", Fixture::Banana);
    press(&mut app, PageId::Audio, |a| *a == Action::PickSource(1));
    assert_eq!(pick(&app), PickMode::Picked(1));
    press(&mut app, PageId::Audio, |a| *a == Action::CancelPick);
    assert_eq!(pick(&app), PickMode::None);
    assert!(app.mailbox.latest().scene.expanded);
}

#[test]
fn media_buttons_and_seek_reach_the_services() {
    let (mut app, _, _) = fed("input-media", Fixture::Banana);
    let playing = facts(&mut app).media.playing;
    press(&mut app, PageId::Media, |a| *a == Action::PlayPause);
    assert_eq!(facts(&mut app).media.playing, !playing);
    app.on_event(ShellEvent::Wake);
    press(&mut app, PageId::Media, |a| {
        matches!(a, Action::SeekRuler(_))
    });
    let timeline = facts(&mut app).media.timeline.expect("timeline");
    let half = timeline.duration_s / 2.0;
    assert!(
        (timeline.position_s - half).abs() < 2.0,
        "{} near {half}",
        timeline.position_s
    );
}

#[test]
fn system_page_writes_brightness_and_the_mic() {
    let (mut app, _, _) = fed("input-system", Fixture::Banana);
    let before = facts(&mut app).system;
    press(&mut app, PageId::System, |a| *a == Action::ToggleMic);
    press(&mut app, PageId::System, |a| {
        matches!(a, Action::BrightnessRuler(_))
    });
    let after = facts(&mut app).system;
    assert_ne!(
        after.mic.as_ref().map(|m| m.muted),
        before.mic.as_ref().map(|m| m.muted)
    );
    assert_eq!(after.brightness.map(|b| b.1), Some(50));
}

#[test]
fn leaving_collapses_after_the_delay_and_ends_the_pick() {
    let (mut app, timers, _, seconds) = clocked("input-leave", Fixture::Banana);
    press(&mut app, PageId::Audio, |a| *a == Action::PickSource(0));
    app.on_event(ShellEvent::Leave);
    assert!(running(&timers).contains(&COLLAPSE_TIMER));
    seconds.set(0.2);
    app.on_event(ShellEvent::Timer(COLLAPSE_TIMER));
    assert!(app.mailbox.latest().scene.expanded, "too early");
    assert!(running(&timers).contains(&COLLAPSE_TIMER), "still waiting");
    seconds.set(0.4);
    app.on_event(ShellEvent::Timer(COLLAPSE_TIMER));
    assert!(!app.mailbox.latest().scene.expanded);
    assert_eq!(pick(&app), PickMode::None);
    assert!(running(&timers).is_empty(), "collapsed: no timers at all");
}

#[test]
fn coming_back_before_the_delay_keeps_it_open() {
    let (mut app, timers, _, seconds) = clocked("input-back", Fixture::Banana);
    app.on_event(CLICK);
    app.on_event(ShellEvent::Leave);
    seconds.set(0.3);
    app.on_event(ShellEvent::Enter);
    assert!(!running(&timers).contains(&COLLAPSE_TIMER));
    seconds.set(1.0);
    app.on_event(ShellEvent::Timer(COLLAPSE_TIMER));
    assert!(
        app.mailbox.latest().scene.expanded,
        "a stray tick does nothing"
    );
}

#[test]
fn leaving_a_collapsed_island_arms_nothing() {
    let (mut app, timers, _, seconds) = clocked("input-closed", Fixture::Banana);
    app.on_event(ShellEvent::Leave);
    assert!(running(&timers).is_empty());
    seconds.set(1.0);
    let generation = app.mailbox.latest().generation;
    app.on_event(ShellEvent::Timer(COLLAPSE_TIMER));
    assert_eq!(app.mailbox.latest().generation, generation);
}

#[test]
fn a_tick_while_the_scene_is_closed_waits_for_the_wake() {
    let (mut app, _, minute) = fed("feed-closed-tick", Fixture::Banana);
    app.on_event(CLICK);
    // Another publisher (the demo) collapses; its wake has not arrived yet.
    let _ = app.mailbox.update(|scene| Scene {
        expanded: false,
        ..scene
    });
    minute.set(48);
    let generation = app.mailbox.latest().generation;
    for id in [ACTIVITY_TIMER, MEDIA_TIMER, CLOCK_TIMER] {
        app.on_event(ShellEvent::Timer(id));
    }
    assert_eq!(app.mailbox.latest().generation, generation);
}
