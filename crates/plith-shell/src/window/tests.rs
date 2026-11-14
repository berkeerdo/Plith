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
    let first = guard("window procedure", || inner.emit(ShellEvent::Click));
    inner.fail(first.expect_err("the handler panics"));
    inner.fail(String::from("a second report"));
    inner.emit(ShellEvent::Closing);
    assert_eq!(entered.get(), 1, "the panicking handler is not re-entered");
    assert_eq!(
        *reports.borrow(),
        vec!["window procedure panicked: handler broke".to_string()]
    );
}
