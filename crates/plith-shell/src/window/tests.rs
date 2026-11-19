use super::mouse::PressEnd;
use super::*;
use plith_core::layout::{RectPx, SurfaceLayout};
use plith_core::scene::ShapeCell;
use std::cell::RefCell;

#[test]
fn guard_turns_panic_into_logged_error() {
    let log = RefCell::new(Vec::new());
    assert_eq!(guard("quiet", || {}), Ok(()));
    if let Err(message) = guard("window procedure", || panic!("boom {}", 7)) {
        log.borrow_mut().push(format!("ui fatal: {message}"));
    }
    if let Err(message) = guard("foreground hook", || std::panic::panic_any(5_u8)) {
        log.borrow_mut().push(message);
    }
    assert_eq!(
        *log.borrow(),
        vec![
            "ui fatal: window procedure panicked: boom 7".to_string(),
            "foreground hook panicked: non-string payload".to_string(),
        ]
    );
}

#[test]
fn a_panic_is_reported_once_and_mutes_the_handler() {
    let layout = SurfaceLayout::new(
        RectPx {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        },
        0,
        1.0,
    );
    let reports = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&reports);
    let entered = Rc::new(std::cell::Cell::new(0_u32));
    let counter = Rc::clone(&entered);
    let inner = Inner::new(
        layout,
        Arc::new(ShapeCell::default()),
        Box::new(move |_| {
            counter.set(counter.get() + 1);
            panic!("handler broke");
        }),
        Box::new(move |message| sink.borrow_mut().push(message)),
    );
    let first = guard("window procedure", || {
        inner.emit(ShellEvent::Click { x: 0.0, y: 0.0 });
    });
    inner.fail(first.expect_err("the handler panics"));
    inner.fail(String::from("a second report"));
    inner.emit(ShellEvent::Closing);
    assert_eq!(entered.get(), 1, "the panicking handler is not re-entered");
    assert_eq!(
        *reports.borrow(),
        vec!["window procedure panicked: handler broke".to_string()]
    );
}

fn layout_at(scale: f64) -> SurfaceLayout {
    SurfaceLayout::new(
        RectPx {
            x: 0,
            y: 0,
            width: 2880,
            height: 1620,
        },
        0,
        scale,
    )
}

#[test]
fn click_point_is_island_dip() {
    use plith_core::shapes::{PAGES, REST};
    let layout = layout_at(1.5);
    let (ox, oy) = layout.island_origin(PAGES[0]);
    let px = |x: f64, y: f64| ((ox + x) * 1.5, (oy + y) * 1.5);
    let (x, y) = mouse::island_point(&layout, PAGES[0], px(100.0, 50.0)).expect("inside");
    assert!(
        (x - 100.0).abs() < 1e-9 && (y - 50.0).abs() < 1e-9,
        "{x} {y}"
    );
    assert_eq!(
        mouse::island_point(&layout, PAGES[0], px(-3.0, 50.0)),
        None,
        "left of the island"
    );
    let (rx, ry) = layout.island_origin(REST);
    assert_eq!(
        mouse::island_point(&layout, REST, ((rx - 1.0) * 1.5, (ry + 5.0) * 1.5)),
        None
    );
}

fn recording() -> (Inner, Rc<RefCell<Vec<ShellEvent>>>) {
    let events = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&events);
    let inner = Inner::new(
        layout_at(1.0),
        Arc::new(ShapeCell::default()),
        Box::new(move |event| sink.borrow_mut().push(event)),
        Box::new(|_| {}),
    );
    (inner, events)
}

#[test]
fn first_move_enters_and_leave_leaves() {
    let (inner, events) = recording();
    assert!(mouse::note_move(&inner), "the first move asks for tracking");
    assert!(!mouse::note_move(&inner), "tracking already asked for");
    mouse::note_leave(&inner);
    assert!(
        mouse::note_move(&inner),
        "a move after a leave enters again"
    );
    assert_eq!(
        *events.borrow(),
        vec![ShellEvent::Enter, ShellEvent::Leave, ShellEvent::Enter]
    );
}

#[test]
fn a_move_reports_a_new_point_inside_the_shape_once() {
    use plith_core::page::PageLayers;
    use plith_core::scene::ShapeFrame;
    use plith_core::shapes::REST;
    let (inner, events) = recording();
    let lparam_at = |dx: f64, dy: f64| {
        let (ox, oy) = inner.layout.island_origin(REST);
        // Allowed: the island point sits well inside the 1920 px wide test surface.
        #[allow(clippy::cast_possible_truncation)]
        let (x, y) = ((ox + dx).round() as i16, (oy + dy).round() as i16);
        LPARAM((isize::from(y) << 16) | isize::from(x))
    };
    mouse::note_point(&inner, lparam_at(50.0, 10.0));
    assert!(events.borrow().is_empty(), "no shape published yet");
    inner.shapes.publish(ShapeFrame {
        shape: REST,
        previous: REST,
        generation: 1,
        pages: PageLayers::empty(),
    });
    mouse::note_point(&inner, lparam_at(50.0, 10.0));
    mouse::note_point(&inner, lparam_at(50.0, 10.0));
    mouse::note_point(&inner, lparam_at(-500.0, 10.0));
    mouse::note_point(&inner, lparam_at(51.0, 10.0));
    mouse::note_leave(&inner);
    mouse::note_point(&inner, lparam_at(51.0, 10.0));
    let got = events.borrow();
    let moves: Vec<_> = got
        .iter()
        .filter(|e| matches!(e, ShellEvent::Move { .. }))
        .collect();
    assert_eq!(moves.len(), 3, "{got:?}");
}

#[test]
fn a_click_carries_its_point() {
    let (inner, events) = recording();
    inner.emit(ShellEvent::Click { x: 1.0, y: 2.0 });
    assert_eq!(*events.borrow(), vec![ShellEvent::Click { x: 1.0, y: 2.0 }]);
}

#[test]
fn a_click_asks_for_leave_tracking_again() {
    use plith_core::page::PageLayers;
    use plith_core::scene::ShapeFrame;
    use plith_core::shapes::REST;
    let (inner, events) = recording();
    inner.shapes.publish(ShapeFrame {
        shape: REST,
        previous: REST,
        generation: 1,
        pages: PageLayers::empty(),
    });
    assert!(mouse::note_move(&inner));
    let (ox, oy) = inner.layout.island_origin(REST);
    // Allowed: the island point sits well inside the 1920 px wide test surface.
    #[allow(clippy::cast_possible_truncation)]
    let x = (ox + 50.0).round() as i16;
    #[allow(clippy::cast_possible_truncation)]
    let y = (oy + 10.0).round() as i16;
    let lparam = LPARAM((isize::from(y) << 16) | isize::from(x));
    assert!(mouse::note_down(&inner, lparam), "a press inside the shape");
    mouse::note_up(&inner, lparam);
    assert!(
        mouse::note_move(&inner),
        "the move after an expanding click re-requests tracking"
    );
    let got = events.borrow();
    assert!(matches!(got[1], ShellEvent::Press { .. }), "{got:?}");
    assert!(matches!(got[2], ShellEvent::Click { .. }), "{got:?}");
    assert_eq!(got.len(), 4);
}

const P: (f64, f64) = (10.0, 5.0);

#[test]
fn press_outside_then_release_inside_is_no_click() {
    let press = mouse::PressRecord::default();
    assert!(
        !press.down(None),
        "a press outside the shape takes no capture"
    );
    assert_eq!(press.up(Some(P), P), PressEnd::Nothing);
}

#[test]
fn press_inside_then_release_inside_clicks_at_the_release() {
    let press = mouse::PressRecord::default();
    assert!(
        press.down(Some((4.0, 2.0))),
        "a press inside takes the capture"
    );
    assert_eq!(press.up(Some(P), P), PressEnd::Click(P));
    assert_eq!(
        press.up(Some(P), P),
        PressEnd::Nothing,
        "one press, one click"
    );
}

#[test]
fn release_with_no_press_is_nothing() {
    let press = mouse::PressRecord::default();
    assert_eq!(press.up(Some(P), P), PressEnd::Nothing);
}

#[test]
fn capture_lost_between_press_and_release_is_nothing() {
    let press = mouse::PressRecord::default();
    assert!(press.down(Some((4.0, 2.0))));
    press.cancel();
    assert_eq!(press.up(Some(P), P), PressEnd::Nothing);
}

#[test]
fn press_inside_then_release_outside_is_nothing() {
    let press = mouse::PressRecord::default();
    assert!(press.down(Some((4.0, 2.0))));
    assert_eq!(press.up(None, (400.0, 5.0)), PressEnd::Nothing);
    assert_eq!(
        press.up(Some(P), P),
        PressEnd::Nothing,
        "the record is gone"
    );
}

#[test]
fn small_move_stays_click() {
    let press = mouse::PressRecord::default();
    assert!(press.down(Some((10.0, 10.0))));
    assert_eq!(press.moved((12.0, 11.0)), None, "inside the slop");
    assert_eq!(
        press.up(Some((12.0, 11.0)), (12.0, 11.0)),
        PressEnd::Click((12.0, 11.0))
    );
}

#[test]
fn move_past_slop_drags() {
    let press = mouse::PressRecord::default();
    assert!(press.down(Some((10.0, 10.0))));
    assert_eq!(press.moved((14.0, 10.0)), Some((14.0, 10.0)));
    assert_eq!(press.moved((14.0, 10.0)), None, "same point twice");
    assert_eq!(
        press.up(Some((14.0, 10.0)), (14.0, 10.0)),
        PressEnd::Release {
            at: (14.0, 10.0),
            leave: false
        }
    );
}

#[test]
fn the_slop_is_euclidean_and_strict() {
    let press = mouse::PressRecord::default();
    assert!(press.down(Some((0.0, 0.0))));
    assert_eq!(press.moved((3.0, 0.0)), None, "exactly the slop");
    assert!(press.moved((2.5, 2.5)).is_some(), "diagonal 3.54 is past");
}

#[test]
fn drag_outside_shape_continues() {
    let press = mouse::PressRecord::default();
    assert!(press.down(Some((10.0, 10.0))));
    assert!(press.moved((20.0, 10.0)).is_some());
    assert_eq!(press.moved((-80.0, 400.0)), Some((-80.0, 400.0)));
    assert_eq!(
        press.up(None, (-90.0, 410.0)),
        PressEnd::Release {
            at: (-90.0, 410.0),
            leave: false
        }
    );
}

#[test]
fn a_drag_that_returns_to_the_press_point_stays_a_drag() {
    let press = mouse::PressRecord::default();
    assert!(press.down(Some((10.0, 10.0))));
    assert!(press.moved((30.0, 10.0)).is_some());
    assert_eq!(press.moved((10.0, 10.0)), Some((10.0, 10.0)));
    assert!(matches!(
        press.up(Some((10.0, 10.0)), (10.0, 10.0)),
        PressEnd::Release { .. }
    ));
}

#[test]
fn leave_during_drag_is_deferred() {
    let press = mouse::PressRecord::default();
    assert!(press.down(Some((10.0, 10.0))));
    assert!(press.moved((30.0, 10.0)).is_some());
    assert!(press.leave(), "deferred while dragging");
    assert_eq!(
        press.up(None, (500.0, 10.0)),
        PressEnd::Release {
            at: (500.0, 10.0),
            leave: true
        }
    );
}

#[test]
fn leave_then_release_inside_reports_no_leave() {
    let press = mouse::PressRecord::default();
    assert!(press.down(Some((10.0, 10.0))));
    assert!(press.moved((30.0, 10.0)).is_some());
    assert!(press.leave());
    assert_eq!(
        press.up(Some((20.0, 10.0)), (20.0, 10.0)),
        PressEnd::Release {
            at: (20.0, 10.0),
            leave: false
        }
    );
}

#[test]
fn leave_before_a_drag_cancels_the_press() {
    let press = mouse::PressRecord::default();
    assert!(press.down(Some((10.0, 10.0))));
    assert!(!press.leave(), "no drag: not deferred");
    assert_eq!(press.up(Some(P), P), PressEnd::Nothing);
}

#[test]
fn lost_capture_ends_a_drag_at_its_last_point() {
    let press = mouse::PressRecord::default();
    assert!(press.down(Some((10.0, 10.0))));
    assert_eq!(press.lost(), None, "a plain press just ends");
    assert!(press.down(Some((10.0, 10.0))));
    assert!(press.moved((30.0, 12.0)).is_some());
    assert!(press.leave());
    assert_eq!(press.lost(), Some(((30.0, 12.0), true)));
    assert_eq!(press.lost(), None);
}

#[test]
fn a_release_without_a_press_emits_nothing() {
    let (inner, events) = recording();
    mouse::note_up(&inner, LPARAM(0));
    assert!(events.borrow().is_empty());
}

/// The `lparam` of the client pixel at island point `(dx, dy)` (scale 1.0).
fn lparam_island(inner: &Inner, dx: f64, dy: f64) -> LPARAM {
    let (ox, oy) = inner.layout.island_origin(plith_core::shapes::REST);
    // Allowed: the test points sit well inside the signed 16-bit client range.
    #[allow(clippy::cast_possible_truncation)]
    let (x, y) = ((ox + dx).round() as i16, (oy + dy).round() as i16);
    LPARAM((isize::from(y) << 16) | (isize::from(x) & 0xFFFF))
}

fn publish_rest(inner: &Inner) {
    use plith_core::page::PageLayers;
    use plith_core::scene::ShapeFrame;
    use plith_core::shapes::REST;
    inner.shapes.publish(ShapeFrame {
        shape: REST,
        previous: REST,
        generation: 1,
        pages: PageLayers::empty(),
    });
}

#[test]
fn a_drag_reports_press_drag_release_then_the_deferred_leave() {
    let (inner, events) = recording();
    publish_rest(&inner);
    assert!(mouse::note_down(&inner, lparam_island(&inner, 50.0, 10.0)));
    mouse::note_point(&inner, lparam_island(&inner, 52.0, 10.0));
    mouse::note_point(&inner, lparam_island(&inner, 80.0, 10.0));
    mouse::note_point(&inner, lparam_island(&inner, 80.0, 300.0));
    mouse::note_leave(&inner);
    assert!(
        !events.borrow().contains(&ShellEvent::Leave),
        "the leave waits for the release"
    );
    mouse::note_up(&inner, lparam_island(&inner, 80.0, 310.0));
    let got = events.borrow();
    assert_eq!(got.first(), Some(&ShellEvent::Press { x: 50.0, y: 10.0 }));
    assert!(
        got.contains(&ShellEvent::Drag { x: 80.0, y: 300.0 }),
        "{got:?}"
    );
    assert!(!got.iter().any(|e| matches!(e, ShellEvent::Click { .. })));
    let tail = &got[got.len() - 2..];
    assert_eq!(
        tail,
        [ShellEvent::Release { x: 80.0, y: 310.0 }, ShellEvent::Leave]
    );
}

#[test]
fn a_press_that_stays_in_the_slop_clicks_and_never_drags() {
    let (inner, events) = recording();
    publish_rest(&inner);
    assert!(mouse::note_down(&inner, lparam_island(&inner, 50.0, 10.0)));
    mouse::note_point(&inner, lparam_island(&inner, 52.0, 11.0));
    mouse::note_up(&inner, lparam_island(&inner, 52.0, 11.0));
    let got = events.borrow();
    assert!(!got.iter().any(|e| matches!(e, ShellEvent::Drag { .. })));
    assert_eq!(got.last(), Some(&ShellEvent::Click { x: 52.0, y: 11.0 }));
}

#[test]
fn a_lost_capture_during_a_drag_releases_at_the_last_point() {
    let (inner, events) = recording();
    publish_rest(&inner);
    assert!(mouse::note_down(&inner, lparam_island(&inner, 50.0, 10.0)));
    mouse::note_point(&inner, lparam_island(&inner, 90.0, 10.0));
    mouse::note_leave(&inner);
    mouse::note_capture_lost(&inner);
    let got = events.borrow();
    let tail = &got[got.len() - 2..];
    assert_eq!(
        tail,
        [ShellEvent::Release { x: 90.0, y: 10.0 }, ShellEvent::Leave]
    );
}
