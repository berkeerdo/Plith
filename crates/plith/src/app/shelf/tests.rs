use super::*;
use crate::app::feed::{COLLAPSE_TIMER, Timers};
use crate::log::{LogFile, Stamp};
use crate::services::fixture::FixtureServices;
use plith_core::draw::Node;
use plith_core::lang::Lang;
use plith_core::launch::Fixture;
use plith_core::scene::{Scene, SceneMailbox, ShapeCell};
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

#[derive(Clone, Default)]
struct FakeTimers(Rc<RefCell<BTreeMap<usize, u32>>>);

impl Timers for FakeTimers {
    fn set(&self, id: usize, ms: u32) {
        self.0.borrow_mut().insert(id, ms);
    }

    fn kill(&self, id: usize) {
        self.0.borrow_mut().remove(&id);
    }
}

struct Rig {
    app: App,
    timers: FakeTimers,
    now: Rc<Cell<f64>>,
    log_path: PathBuf,
}

fn stamp() -> Stamp {
    Stamp {
        year: 2026,
        month: 11,
        day: 24,
        hour: 21,
        minute: 47,
        second: 10,
        millis: 0,
    }
}

fn rig(name: &str) -> Rig {
    let dir = std::env::temp_dir().join(format!("plith-shelf-app-{name}-{}", std::process::id()));
    let log_path = dir.join("plith.log");
    let log = LogFile::open_at(&log_path).unwrap();
    let mailbox = Arc::new(SceneMailbox::new(Scene::default(), Arc::default()));
    let now = Rc::new(Cell::new(10.0));
    let clock = {
        let now = Rc::clone(&now);
        Box::new(move || now.get())
    };
    let mut app = App::new(
        Arc::clone(&mailbox),
        Arc::new(ShapeCell::default()),
        Arc::new(log),
        Arc::new(AtomicBool::new(false)),
    )
    .with_clock(clock);
    let timers = FakeTimers::default();
    let feed = Feed::new(
        Box::new(FixtureServices::new(Fixture::Banana)),
        Box::new(timers.clone()),
        Box::new(stamp),
        Lang::Tr,
        &mailbox,
    );
    app.attach_feed(feed);
    Rig {
        app,
        timers,
        now,
        log_path,
    }
}

impl Rig {
    fn advance(&self, seconds: f64) {
        self.now.set(self.now.get() + seconds);
    }

    fn timer_ms(&self, id: usize) -> Option<u32> {
        self.timers.0.borrow().get(&id).copied()
    }

    fn page(&self) -> usize {
        self.app.mailbox.latest().scene.page
    }

    fn expanded(&self) -> bool {
        self.app.mailbox.latest().scene.expanded
    }

    fn log_text(&self) -> String {
        self.app.log.flush();
        std::fs::read_to_string(&self.log_path).unwrap_or_default()
    }

    fn shelf_has_hot_slot(&self) -> bool {
        let content = self.app.mailbox.latest().content;
        content.pages[PageId::Shelf.index()].nodes.iter().any(|n| {
            matches!(
                n,
                Node::Stroke {
                    dashed: true,
                    ink: plith_core::draw::Ink::Flow,
                    ..
                }
            )
        })
    }
}

fn enter(rig: &mut Rig) {
    rig.app
        .on_drag_enter("hdrop,FileNameW".to_owned(), "copy", (310.0, 12.0));
}

#[test]
fn a_file_drag_arms_the_dwell_and_opens_nothing_yet() {
    let mut rig = rig("dwell");
    enter(&mut rig);
    assert_eq!(rig.timer_ms(SHELF_TIMER), Some(150));
    assert!(!rig.expanded());
}

#[test]
fn the_dwell_opens_the_island_on_the_shelf_with_a_hot_slot() {
    let mut rig = rig("open");
    enter(&mut rig);
    rig.advance(0.152);
    rig.app.on_shelf_tick();
    assert!(rig.expanded());
    assert_eq!(rig.page(), PageId::Shelf.index());
    assert!(rig.shelf_has_hot_slot());
    assert_eq!(rig.timer_ms(SHELF_TIMER), None);
}

#[test]
fn a_drag_that_leaves_before_the_dwell_opens_nothing() {
    let mut rig = rig("cross");
    enter(&mut rig);
    rig.advance(0.05);
    rig.app.on_drag_leave();
    rig.advance(0.2);
    rig.app.on_shelf_tick();
    assert!(!rig.expanded());
    assert!(
        rig.log_text()
            .contains("end=left files=0 effect=copy point=310,12 enters=1 open_ms=none")
    );
}

#[test]
fn an_open_shelf_waits_out_the_grace_then_starts_the_normal_collapse() {
    let mut rig = rig("grace");
    enter(&mut rig);
    rig.advance(0.15);
    rig.app.on_shelf_tick();
    rig.app.on_drag_leave();
    assert_eq!(rig.timer_ms(SHELF_TIMER), Some(380));
    assert!(!rig.shelf_has_hot_slot());
    assert_eq!(rig.timer_ms(COLLAPSE_TIMER), None);
    rig.advance(0.38);
    rig.app.on_shelf_tick();
    assert_eq!(rig.timer_ms(COLLAPSE_TIMER), Some(400));
    assert!(rig.expanded(), "the collapse itself waits its own delay");
}

#[test]
fn coming_back_inside_the_grace_keeps_the_shelf_open() {
    let mut rig = rig("back");
    enter(&mut rig);
    rig.advance(0.15);
    rig.app.on_shelf_tick();
    rig.app.on_drag_leave();
    rig.advance(0.1);
    enter(&mut rig);
    rig.advance(0.5);
    rig.app.on_shelf_tick();
    assert_eq!(rig.timer_ms(COLLAPSE_TIMER), None);
    assert!(rig.shelf_has_hot_slot());
}

#[test]
fn a_drop_puts_the_files_on_the_shelf_and_logs_one_line() {
    let mut rig = rig("drop");
    let dir = std::env::temp_dir().join(format!("plith-shelf-drop-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("a.png");
    std::fs::write(&file, "x").unwrap();
    enter(&mut rig);
    rig.advance(0.15);
    rig.app.on_shelf_tick();
    rig.app.on_drop(&[file.clone(), dir.clone()]);
    let items = rig.app.feed.as_ref().unwrap().shelf_items().to_vec();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].path, file);
    assert!(items[1].dir);
    assert!(!rig.shelf_has_hot_slot());
    let log = rig.log_text();
    assert!(
        log.contains("end=drop files=2 effect=copy point=310,12 enters=1 open_ms=150"),
        "{log}"
    );
    assert!(log.contains("formats=hdrop,FileNameW"));
}

#[test]
fn a_drop_before_the_dwell_still_opens_the_shelf() {
    let mut rig = rig("early");
    enter(&mut rig);
    rig.advance(0.05);
    rig.app.on_drop(&[std::env::temp_dir()]);
    assert!(rig.expanded());
    assert_eq!(rig.page(), PageId::Shelf.index());
}

#[test]
fn a_drag_without_files_is_logged_and_opens_nothing() {
    let mut rig = rig("refused");
    rig.app.on_drag_refused("unicodetext".to_owned());
    assert!(!rig.expanded());
    assert_eq!(rig.timer_ms(SHELF_TIMER), None);
    let log = rig.log_text();
    assert!(log.contains(
        "end=refused files=0 effect=none point=0,0 enters=1 open_ms=none formats=unicodetext"
    ));
}

fn report(effect: &'static str, inside: bool) -> DragOutReport {
    DragOutReport {
        offered: 1,
        missing: 0,
        effect,
        ms: 840,
        inside,
    }
}

#[test]
fn a_pressed_wagon_that_passes_the_slop_runs_the_drag_out_once() {
    let mut rig = rig("out");
    let calls = Rc::new(Cell::new(0));
    let seen = Rc::new(RefCell::new(Vec::<PathBuf>::new()));
    let (c, s) = (Rc::clone(&calls), Rc::clone(&seen));
    rig.app.shelf = ShelfHost::with_runner(Box::new(move |paths| {
        c.set(c.get() + 1);
        s.borrow_mut().extend(paths.iter().cloned());
        report("copy", true)
    }));
    rig.app.shelf.pressed = Some(PathBuf::from("C:\\a\\b.png"));
    assert!(rig.app.take_wagon_drag());
    assert!(!rig.app.take_wagon_drag(), "the press is used up");
    assert_eq!(calls.get(), 1);
    assert_eq!(*seen.borrow(), [PathBuf::from("C:\\a\\b.png")]);
    assert!(
        rig.log_text()
            .contains("drag: dir=out items=1 missing=0 effect=copy ms=840")
    );
}

#[test]
fn a_return_inside_the_grace_keeps_the_open_time_and_counts_the_enter() {
    let mut rig = rig("chatter");
    enter(&mut rig);
    rig.advance(0.15);
    rig.app.on_shelf_tick();
    rig.app.on_drag_leave();
    rig.advance(0.1);
    enter(&mut rig);
    rig.app.on_drop(&[std::env::temp_dir()]);
    let log = rig.log_text();
    assert!(
        log.contains("end=drop files=1 effect=copy point=310,12 enters=2 open_ms=150"),
        "{log}"
    );
}

#[test]
fn a_thousand_dropped_paths_leave_eight_on_the_shelf() {
    let mut rig = rig("big");
    let paths: Vec<PathBuf> = (0..1000)
        .map(|i| PathBuf::from(format!("C:\\nowhere\\f{i}.txt")))
        .collect();
    enter(&mut rig);
    rig.app.on_drop(&paths);
    let items = rig.app.feed.as_ref().unwrap().shelf_items().to_vec();
    assert_eq!(items.len(), 8);
    assert_eq!(items[0].path, paths[0]);
}

#[test]
fn nothing_is_saved_until_the_saved_shelf_arrived_then_both_are_kept() {
    let mut rig = rig("load");
    let saves = Rc::new(RefCell::new(Vec::<usize>::new()));
    let seen = Rc::clone(&saves);
    rig.app
        .feed
        .as_mut()
        .unwrap()
        .begin_shelf_load(Box::new(move |shelf| seen.borrow_mut().push(shelf.len())));
    let (tx, rx) = std::sync::mpsc::channel();
    rig.app.attach_shelf_load(rx);
    enter(&mut rig);
    rig.app.on_drop(&[PathBuf::from("C:\\nowhere\\new.txt")]);
    assert!(saves.borrow().is_empty(), "a save now would erase the file");
    let mut old = plith_core::shelf::Shelf::default();
    old.add(&[ShelfItem {
        path: PathBuf::from("\\\\nas\\share\\old.txt"),
        dir: false,
    }]);
    tx.send(old).unwrap();
    rig.app.drain_shelf_load();
    assert_eq!(*saves.borrow(), [2]);
    let items = rig.app.feed.as_ref().unwrap().shelf_items().to_vec();
    assert_eq!(items[0].path, PathBuf::from("C:\\nowhere\\new.txt"));
}

#[test]
fn a_drag_with_no_wagon_pressed_is_not_taken() {
    let mut rig = rig("none");
    assert!(!rig.app.take_wagon_drag());
}

#[test]
fn the_island_ignores_file_drags_while_its_own_drag_runs() {
    let mut rig = rig("own");
    rig.app.shelf.hover.set_own(true);
    enter(&mut rig);
    rig.advance(0.5);
    rig.app.on_shelf_tick();
    assert_eq!(rig.timer_ms(SHELF_TIMER), None, "own drag arms nothing");
    assert!(!rig.expanded());
}

#[test]
fn a_drag_out_that_ends_outside_starts_the_collapse() {
    let mut rig = rig("outside");
    rig.app.expand_on(Some(PageId::Shelf));
    rig.app.shelf = ShelfHost::with_runner(Box::new(|_| report("copy", false)));
    rig.app.shelf.pressed = Some(PathBuf::from("C:\\a\\b.png"));
    assert!(rig.app.take_wagon_drag());
    assert_eq!(rig.timer_ms(COLLAPSE_TIMER), Some(400));
}

#[test]
fn a_drag_out_that_ends_over_the_island_leaves_it_open() {
    let mut rig = rig("inside");
    rig.app.expand_on(Some(PageId::Shelf));
    rig.app.shelf = ShelfHost::with_runner(Box::new(|_| report("copy", true)));
    rig.app.shelf.pressed = Some(PathBuf::from("C:\\a\\b.png"));
    assert!(rig.app.take_wagon_drag());
    assert_eq!(rig.timer_ms(COLLAPSE_TIMER), None);
}
