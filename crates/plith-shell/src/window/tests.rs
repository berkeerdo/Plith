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
fn a_click_carries_its_point() {
    let (inner, events) = recording();
    inner.emit(ShellEvent::Click { x: 1.0, y: 2.0 });
    assert_eq!(*events.borrow(), vec![ShellEvent::Click { x: 1.0, y: 2.0 }]);
}
