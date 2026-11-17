use super::*;
use crate::app::feed::{Feed, Timers};
use crate::log::{LogFile, Stamp};
use crate::services::fixture::FixtureServices;
use plith_core::launch::Fixture;
use plith_core::pages::welcome::{WelcomeFacts, probe_hour};
use plith_core::scene::ShapeCell;
use plith_shell::ShellEvent;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

struct NoTimers;

impl Timers for NoTimers {
    fn set(&self, _id: usize, _ms: u32) {}
    fn kill(&self, _id: usize) {}
}

fn welcome_facts() -> WelcomeFacts {
    use plith_core::lang::Lang;
    use plith_core::settings::{HeadSlot, RunSpan, TempUnit};
    WelcomeFacts {
        now: ((2026, 11, 17), 14 * 60 + 32),
        date: "SAL 17 KAS".into(),
        plays: Vec::new(),
        playing: false,
        forecast: None,
        weather_on: false,
        span: RunSpan::Six,
        unit: TempUnit::C,
        head: HeadSlot::Auto,
        sound: None,
        elapsed: None,
        lang: Lang::Tr,
    }
}

/// A fed app whose welcome facts are set; `open_on` puts the scene on `page`.
fn app_with_welcome(name: &str) -> App {
    let dir = std::env::temp_dir().join(format!("plith-input-{name}-{}", std::process::id()));
    let log = LogFile::open_at(&dir.join("plith.log")).unwrap();
    let mailbox = Arc::new(SceneMailbox::new(Scene::default(), Arc::default()));
    let mut app = App::new(
        Arc::clone(&mailbox),
        Arc::new(ShapeCell::default()),
        Arc::new(log),
        Arc::new(AtomicBool::new(false)),
    );
    let clock = Box::new(|| Stamp {
        year: 2026,
        month: 11,
        day: 17,
        hour: 14,
        minute: 32,
        second: 10,
        millis: 0,
    });
    let mut feed = Feed::new(
        Box::new(FixtureServices::new(Fixture::Banana)),
        Box::new(NoTimers),
        clock,
        &mailbox,
    );
    feed.set_welcome_facts(Some(welcome_facts()), &mailbox);
    app.attach_feed(feed);
    app
}

fn open_on(app: &App, page: PageId) {
    let _ = app.mailbox.update(|scene| Scene {
        expanded: true,
        page: page.index(),
        ..scene
    });
}

fn probe(app: &App) -> Option<i32> {
    app.feed.as_ref().unwrap().probe()
}

/// A point inside the run band at the given x.
fn at(x: f64) -> ShellEvent {
    ShellEvent::Move { x, y: 150.0 }
}

#[test]
fn the_test_points_fall_in_hour_10() {
    let f = welcome_facts();
    assert_eq!(probe_hour(&f, 100.0, 150.0), Some(10));
    assert_eq!(probe_hour(&f, 140.0, 150.0), Some(10));
    assert_eq!(probe_hour(&f, 200.0, 150.0), Some(12));
}

#[test]
fn moves_within_one_hour_cell_rebuild_once() {
    let mut app = app_with_welcome("one-cell");
    open_on(&app, PageId::Welcome);
    let before = app.mailbox.latest().generation;
    app.on_event(at(100.0));
    let first = app.mailbox.latest().generation;
    assert!(first > before, "entering a cell rebuilds the page");
    assert_eq!(probe(&app), Some(10));
    app.on_event(at(120.0));
    app.on_event(at(140.0));
    assert_eq!(
        app.mailbox.latest().generation,
        first,
        "same cell, no rebuild"
    );
    app.on_event(at(200.0));
    assert!(
        app.mailbox.latest().generation > first,
        "a new cell rebuilds"
    );
    assert_eq!(probe(&app), Some(12));
}

#[test]
fn leave_clears_probe() {
    let mut app = app_with_welcome("leave");
    open_on(&app, PageId::Welcome);
    app.on_event(at(100.0));
    assert_eq!(probe(&app), Some(10));
    let generation = app.mailbox.latest().generation;
    app.on_event(ShellEvent::Leave);
    assert_eq!(probe(&app), None);
    assert!(app.mailbox.latest().generation > generation);
    let generation = app.mailbox.latest().generation;
    app.on_event(ShellEvent::Leave);
    assert_eq!(
        app.mailbox.latest().generation,
        generation,
        "nothing to clear"
    );
}

#[test]
fn moves_while_collapsed_are_ignored() {
    let mut app = app_with_welcome("collapsed");
    let _ = app.mailbox.update(|scene| Scene {
        page: PageId::Welcome.index(),
        ..scene
    });
    let generation = app.mailbox.latest().generation;
    app.on_event(at(100.0));
    assert_eq!(probe(&app), None);
    assert_eq!(app.mailbox.latest().generation, generation);
}

#[test]
fn moves_on_other_pages_are_ignored() {
    let mut app = app_with_welcome("other-page");
    open_on(&app, PageId::Audio);
    let generation = app.mailbox.latest().generation;
    app.on_event(at(100.0));
    assert_eq!(probe(&app), None);
    assert_eq!(app.mailbox.latest().generation, generation);
}

#[test]
fn a_move_out_of_the_band_clears_the_probe() {
    let mut app = app_with_welcome("out-of-band");
    open_on(&app, PageId::Welcome);
    app.on_event(at(100.0));
    app.on_event(ShellEvent::Move { x: 100.0, y: 20.0 });
    assert_eq!(probe(&app), None);
}
