use super::*;
use crate::app::feed::{CLOCK_TIMER, Timers};
use crate::app::idle_facts::Idle;
use crate::services::fixture::FixtureServices;
use plith_core::launch::Fixture;
use plith_core::scene::{Scene, SceneMailbox, ShapeCell};
use plith_host::log::Stamp;
use plith_shell::ShellEvent;
use std::cell::Cell;
use std::collections::BTreeSet;

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

impl FakeTimers {
    fn clock(&self) -> bool {
        self.0.borrow().contains(&CLOCK_TIMER)
    }
}

fn folder() -> PathBuf {
    std::env::temp_dir().join(format!("plith-sheet-clock-{}", std::process::id()))
}

/// A collapsed app on the banana fixture showing the plain look, its timers and its minute.
fn plain_app() -> (App, FakeTimers, Rc<Cell<u16>>) {
    let log = LogFile::open_at(&folder().join("plith.log")).unwrap();
    let mailbox = Arc::new(SceneMailbox::new(Scene::default(), Arc::default()));
    let mut app = App::new(
        mailbox,
        Arc::new(ShapeCell::default()),
        Arc::new(log),
        Arc::new(std::sync::atomic::AtomicBool::new(false)),
    );
    let timers = FakeTimers::default();
    let minute = Rc::new(Cell::new(5_u16));
    let clock = {
        let minute = Rc::clone(&minute);
        Box::new(move || Stamp {
            year: 2026,
            month: 11,
            day: 20,
            hour: 10,
            minute: minute.get(),
            second: 0,
            millis: 0,
        })
    };
    let mut feed = Feed::new(
        Box::new(FixtureServices::new(Fixture::Banana)),
        Box::new(timers.clone()),
        clock,
        Lang::Tr,
        &app.mailbox,
    );
    let plain = Settings {
        look: IdleLook::Plain,
        ..Settings::default()
    };
    feed.set_idle(Idle::new(plain.clone(), Lang::Tr), &app.mailbox);
    app.attach_feed(feed);
    app.set_settings(plain);
    (app, timers, minute)
}

/// The time in the idle facts the previews are built from.
fn time_of(app: &App) -> String {
    let feed = app.feed.as_ref().unwrap();
    feed.idle_facts().unwrap().time.clone()
}

#[test]
fn the_open_window_runs_the_minute_clock_for_its_previews() {
    let (mut app, timers, minute) = plain_app();
    assert!(!app.mailbox.latest().scene.expanded);
    assert!(!timers.clock(), "collapsed plain look: no clock");

    app.sheet_clock(true);
    assert!(timers.clock(), "the window wants the minute");

    // The tick rebuilds the idle facts although the plain pill shows no time.
    let before = time_of(&app);
    minute.set(6);
    app.on_event(ShellEvent::Timer(CLOCK_TIMER));
    assert_ne!(before, time_of(&app));
    assert!(timers.clock(), "armed again for the next minute");

    app.sheet_clock(false);
    assert!(!timers.clock(), "closed: back to no clock");
    drop(app);
    std::fs::remove_dir_all(folder()).unwrap();
}

fn long_title_facts(scroll_start: Option<std::time::SystemTime>) -> IdleFacts {
    IdleFacts {
        time: String::from("10:05"),
        turn: None,
        now_sky: None,
        playing: Some((
            String::from("A Very Long Song Title That Cannot Fit"),
            String::from("Artist"),
        )),
        seal: plith_core::pages::pill::PillFacts {
            output: String::from("G733"),
            value: Some(62),
            state: plith_core::pages::pill::PillState::Live,
        },
        sound: None,
        lang: Lang::En,
        scroll_start,
    }
}

fn has_live(list: &DisplayList) -> bool {
    list.nodes
        .iter()
        .any(|n| matches!(n, plith_core::draw::Node::Live(_)))
}

#[test]
fn previews_show_the_title_while_its_scroll_runs() {
    let running = long_title_facts(Some(std::time::SystemTime::UNIX_EPOCH));
    let karot = IdleLook::ALL.iter().position(|l| *l == IdleLook::Karot);
    let live = idle_pill(IdleLook::Karot, &running);
    assert!(has_live(&live), "the real pill scrolls");
    let previews = static_previews(&running);
    assert!(previews.iter().all(|list| !has_live(list)));
    assert_eq!(
        previews[karot.unwrap()].as_ref(),
        &idle_pill(IdleLook::Karot, &long_title_facts(None))
    );
}
