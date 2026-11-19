use super::*;
use crate::app::feed::drag::DRAG_TIMER;
use crate::app::feed::{COLLAPSE_TIMER, Timers};
use crate::log::{LogFile, Stamp};
use crate::services::fixture::FixtureServices;
use crate::services::{Changes, Facts, LevelSource, Services};
use plith_core::input::drag::RulerKind;
use plith_core::lang::Lang;
use plith_core::launch::Fixture;
use plith_core::page::{PageLayer, PageLayers};
use plith_core::scene::{ShapeCell, ShapeFrame};
use plith_core::shapes::PAGES;
use plith_shell::ShellEvent;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

/// A write the fixture saw.
#[derive(Debug, Clone, PartialEq)]
enum Write {
    Volume(u8),
    Brightness(u8),
    Seek(Duration),
}

type Writes = Rc<RefCell<Vec<Write>>>;

/// The fixture services, recording the writes a drag makes.
struct Spy {
    inner: FixtureServices,
    writes: Writes,
    /// Reports a track with no length.
    zero_duration: bool,
}

impl Services for Spy {
    fn drain(&mut self) -> Changes {
        self.inner.drain()
    }
    fn facts(&self) -> Facts {
        let mut facts = self.inner.facts();
        if let (true, Some(timeline)) = (self.zero_duration, facts.media.timeline.as_mut()) {
            timeline.duration_s = 0.0;
        }
        facts
    }
    fn level_source(&self) -> LevelSource {
        self.inner.level_source()
    }
    fn set_default_endpoint(&mut self, id: &str) {
        self.inner.set_default_endpoint(id);
    }
    fn toggle_route(&mut self, strip: usize, bus: usize) {
        self.inner.toggle_route(strip, bus);
    }
    fn toggle_mic(&mut self) {
        self.inner.toggle_mic();
    }
    fn set_volume(&mut self, percent: u8) {
        self.writes.borrow_mut().push(Write::Volume(percent));
        self.inner.set_volume(percent);
    }
    fn set_brightness(&mut self, percent: u8) {
        self.writes.borrow_mut().push(Write::Brightness(percent));
        self.inner.set_brightness(percent);
    }
    fn play_pause(&mut self) {
        self.inner.play_pause();
    }
    fn next(&mut self) {
        self.inner.next();
    }
    fn previous(&mut self) {
        self.inner.previous();
    }
    fn seek(&mut self, position: Duration) {
        self.writes.borrow_mut().push(Write::Seek(position));
        self.inner.seek(position);
    }
}

#[derive(Clone, Default)]
struct Armed(Rc<RefCell<Vec<usize>>>);

impl Timers for Armed {
    fn set(&self, id: usize, _ms: u32) {
        let mut ids = self.0.borrow_mut();
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    fn kill(&self, id: usize) {
        self.0.borrow_mut().retain(|armed| *armed != id);
    }
}

struct Rig {
    app: App,
    writes: Writes,
    seconds: Rc<Cell<f64>>,
    timers: Armed,
}

/// Without Voicemeeter: the audio page then shows the Windows default output's volume ruler.
fn rig(name: &str) -> Rig {
    rig_with(name, false)
}

fn rig_with(name: &str, zero_duration: bool) -> Rig {
    let dir = std::env::temp_dir().join(format!("plith-drag-{name}-{}", std::process::id()));
    let log = LogFile::open_at(&dir.join("plith.log")).unwrap();
    let mailbox = Arc::new(SceneMailbox::new(Scene::default(), Arc::default()));
    let seconds = Rc::new(Cell::new(0.0));
    let now = {
        let seconds = Rc::clone(&seconds);
        Box::new(move || seconds.get())
    };
    let mut app = App::new(
        Arc::clone(&mailbox),
        Arc::new(ShapeCell::default()),
        Arc::new(log),
        Arc::new(AtomicBool::new(false)),
    )
    .with_clock(now);
    let writes = Writes::default();
    let timers = Armed::default();
    let clock = Box::new(|| Stamp {
        year: 2026,
        month: 11,
        day: 19,
        hour: 14,
        minute: 32,
        second: 10,
        millis: 0,
    });
    let feed = Feed::new(
        Box::new(Spy {
            inner: FixtureServices::new(Fixture::Bare),
            writes: Rc::clone(&writes),
            zero_duration,
        }),
        Box::new(timers.clone()),
        clock,
        Lang::Tr,
        &mailbox,
    );
    app.attach_feed(feed);
    Rig {
        app,
        writes,
        seconds,
        timers,
    }
}

/// Shows `page` expanded and at rest, as the render thread would publish it.
fn open_on(app: &App, page: PageId) {
    let _ = app.mailbox.update(|scene| Scene {
        expanded: true,
        page: page.index(),
        ..scene
    });
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

/// The ruler of `page` matching `wanted`: its left x, width and centre y.
fn ruler(app: &App, page: PageId, wanted: impl Fn(&Action) -> bool) -> (f64, f64, f64) {
    let feed = app.feed.as_ref().expect("fed");
    let target = feed.targets()[page.index()]
        .iter()
        .find(|t| wanted(&t.action))
        .expect("ruler on the page");
    (
        target.rect.x,
        target.rect.w,
        target.rect.y + target.rect.h / 2.0,
    )
}

fn is_volume(action: &Action) -> bool {
    matches!(action, Action::VolumeRuler(_))
}

fn is_seek(action: &Action) -> bool {
    matches!(action, Action::SeekRuler(_))
}

/// Ten drag points inside 0.1 s along the first 90 percent of the ruler, after a press at its
/// start. Returns the time the last point was sent at.
fn drag_along(rig: &mut Rig, (x0, w, y): (f64, f64, f64)) {
    rig.app.on_event(ShellEvent::Press { x: x0 + 1.0, y });
    for i in 0..10 {
        rig.seconds.set(f64::from(i) * 0.01);
        let x = x0 + w * 0.9 * f64::from(i + 1) / 10.0;
        rig.app.on_event(ShellEvent::Drag { x, y });
    }
}

#[test]
fn drag_volume_writes_throttled_and_final() {
    let mut rig = rig("volume");
    open_on(&rig.app, PageId::Audio);
    let r = ruler(&rig.app, PageId::Audio, is_volume);
    drag_along(&mut rig, r);
    let during = rig.writes.borrow().len();
    assert!((1..=4).contains(&during), "throttled: {during} writes");
    rig.seconds.set(0.1);
    rig.app.on_event(ShellEvent::Release {
        x: r.0 + r.1 * 0.25,
        y: r.2,
    });
    let writes = rig.writes.borrow();
    assert_eq!(writes.len(), during + 1, "the release writes once more");
    assert_eq!(writes.last(), Some(&Write::Volume(25)));
    assert!(writes.iter().all(|w| matches!(w, Write::Volume(_))));
}

#[test]
fn drag_seek_writes_once_on_release() {
    let mut rig = rig("seek");
    open_on(&rig.app, PageId::Media);
    let r = ruler(&rig.app, PageId::Media, is_seek);
    drag_along(&mut rig, r);
    assert!(rig.writes.borrow().is_empty(), "no seek while dragging");
    rig.seconds.set(0.1);
    rig.app.on_event(ShellEvent::Release {
        x: r.0 + r.1 * 0.5,
        y: r.2,
    });
    let writes = rig.writes.borrow();
    assert_eq!(writes.len(), 1);
    let Some(Write::Seek(position)) = writes.first() else {
        panic!("a seek, got {writes:?}");
    };
    let duration = rig.app.feed.as_ref().unwrap().facts().media.timeline;
    let half = duration.expect("timeline").duration_s / 2.0;
    assert!((position.as_secs_f64() - half).abs() < 2.0);
}

#[test]
fn no_seek_ruler_is_built_on_a_zero_length_track() {
    let rig = rig_with("zero-length", true);
    open_on(&rig.app, PageId::Media);
    let feed = rig.app.feed.as_ref().unwrap();
    assert!(
        !feed.targets()[PageId::Media.index()]
            .iter()
            .any(|t| is_seek(&t.action))
    );
}

#[test]
fn a_seek_on_a_zero_length_track_is_not_dispatched() {
    let mut rig = rig_with("zero-dispatch", true);
    let feed = rig.app.feed.as_mut().unwrap();
    assert_eq!(feed.facts().media.timeline.map(|t| t.duration_s), Some(0.0));
    dispatch(feed, &rig.app.mailbox, Action::Seek(0.5));
    assert!(rig.writes.borrow().is_empty());
}

#[test]
fn a_held_still_press_keeps_the_override_until_one_second_after_release() {
    let mut rig = rig("held");
    open_on(&rig.app, PageId::Media);
    let r = ruler(&rig.app, PageId::Media, is_seek);
    rig.app.on_event(ShellEvent::Press {
        x: r.0 + 1.0,
        y: r.2,
    });
    rig.seconds.set(10.0);
    rig.app.on_event(ShellEvent::Drag {
        x: r.0 + r.1 * 0.5,
        y: r.2,
    });
    // Held still past a second: the tick finds the press down and keeps the value.
    rig.seconds.set(12.0);
    rig.app.on_event(ShellEvent::Timer(DRAG_TIMER));
    let feed = rig.app.feed.as_ref().unwrap();
    assert_eq!(feed.override_kind(), Some(RulerKind::Seek));
    assert!(rig.timers.0.borrow().contains(&DRAG_TIMER), "re-armed");
    rig.app.on_event(ShellEvent::Release {
        x: r.0 + r.1 * 0.5,
        y: r.2,
    });
    rig.seconds.set(12.5);
    rig.app.on_event(ShellEvent::Timer(DRAG_TIMER));
    assert_eq!(
        rig.app.feed.as_ref().unwrap().override_kind(),
        Some(RulerKind::Seek)
    );
    rig.seconds.set(13.0);
    rig.app.on_event(ShellEvent::Timer(DRAG_TIMER));
    assert_eq!(rig.app.feed.as_ref().unwrap().override_kind(), None);
}

#[test]
fn override_shows_until_readback() {
    let mut rig = rig("override");
    open_on(&rig.app, PageId::Audio);
    let r = ruler(&rig.app, PageId::Audio, is_volume);
    let before = rig.app.mailbox.latest().content.pages[0].clone();
    rig.app.on_event(ShellEvent::Press {
        x: r.0 + 1.0,
        y: r.2,
    });
    // Dragged to 10 percent: the audio page draws it before any reading came back.
    rig.app.on_event(ShellEvent::Drag {
        x: r.0 + r.1 * 0.1,
        y: r.2,
    });
    let feed = rig.app.feed.as_ref().unwrap();
    assert_eq!(feed.override_kind(), Some(RulerKind::Volume));
    assert_ne!(
        rig.app.mailbox.latest().content.pages[0],
        before,
        "the page shows the drag"
    );
    assert!(rig.timers.0.borrow().contains(&DRAG_TIMER));
    // The reading catches up on the next wake: the override is gone.
    rig.app.on_event(ShellEvent::Wake);
    let feed = rig.app.feed.as_ref().unwrap();
    assert_eq!(feed.override_kind(), None);
    assert!(!rig.timers.0.borrow().contains(&DRAG_TIMER));
}

#[test]
fn override_ends_after_one_second_without_readback() {
    let mut rig = rig("timeout");
    open_on(&rig.app, PageId::Audio);
    let r = ruler(&rig.app, PageId::Audio, is_volume);
    rig.app.on_event(ShellEvent::Press {
        x: r.0 + 1.0,
        y: r.2,
    });
    rig.seconds.set(5.0);
    rig.app.on_event(ShellEvent::Drag {
        x: r.0 + r.1 * 0.1,
        y: r.2,
    });
    rig.app.on_event(ShellEvent::Release {
        x: r.0 + r.1 * 0.1,
        y: r.2,
    });
    rig.seconds.set(5.4);
    rig.app.on_event(ShellEvent::Timer(DRAG_TIMER));
    let feed = rig.app.feed.as_ref().unwrap();
    assert_eq!(feed.override_kind(), Some(RulerKind::Volume), "too early");
    assert!(rig.timers.0.borrow().contains(&DRAG_TIMER), "re-armed");
    rig.seconds.set(6.0);
    rig.app.on_event(ShellEvent::Timer(DRAG_TIMER));
    let feed = rig.app.feed.as_ref().unwrap();
    assert_eq!(feed.override_kind(), None, "one second after the last set");
    assert!(!rig.timers.0.borrow().contains(&DRAG_TIMER));
}

#[test]
fn a_press_off_the_rulers_or_on_a_closed_island_starts_nothing() {
    let mut rig = rig("none");
    rig.app.on_event(ShellEvent::Press { x: 5.0, y: 5.0 });
    rig.app.on_event(ShellEvent::Drag { x: 50.0, y: 5.0 });
    assert!(rig.app.drag.is_none());
    open_on(&rig.app, PageId::Audio);
    let r = ruler(&rig.app, PageId::Audio, is_volume);
    rig.app.on_event(ShellEvent::Press { x: 5.0, y: 5.0 });
    rig.app.on_event(ShellEvent::Drag {
        x: r.0 + r.1 * 0.3,
        y: r.2,
    });
    assert!(rig.writes.borrow().is_empty());
    assert_eq!(rig.app.feed.as_ref().unwrap().override_kind(), None);
}

#[test]
fn drag_leave_does_not_collapse_until_release() {
    let mut rig = rig("leave");
    open_on(&rig.app, PageId::Audio);
    let r = ruler(&rig.app, PageId::Audio, is_volume);
    rig.app.on_event(ShellEvent::Press {
        x: r.0 + 1.0,
        y: r.2,
    });
    // Dragged far outside the island: the shell holds the leave, a stray tick finds nothing.
    rig.app.on_event(ShellEvent::Drag {
        x: r.0 + r.1 * 3.0,
        y: r.2 + 400.0,
    });
    rig.seconds.set(2.0);
    rig.app.on_event(ShellEvent::Timer(COLLAPSE_TIMER));
    assert!(rig.app.mailbox.latest().scene.expanded);
    assert!(!rig.timers.0.borrow().contains(&COLLAPSE_TIMER));
    assert_eq!(rig.writes.borrow().last(), Some(&Write::Volume(100)));
    // The release outside the shape, then the held leave: the delay starts now.
    rig.app.on_event(ShellEvent::Release {
        x: r.0 + r.1 * 3.0,
        y: r.2 + 400.0,
    });
    rig.app.on_event(ShellEvent::Leave);
    assert!(rig.app.mailbox.latest().scene.expanded);
    assert!(rig.timers.0.borrow().contains(&COLLAPSE_TIMER));
    rig.seconds.set(2.5);
    rig.app.on_event(ShellEvent::Timer(COLLAPSE_TIMER));
    assert!(!rig.app.mailbox.latest().scene.expanded);
}
