use super::*;
use crate::monitor::MonitorInfo;
use crate::window::ShellEvent;
use crate::window::state::Inner;
use plith_core::display::Display;
use plith_core::layout::{RectPx, SurfaceLayout};
use plith_core::scene::ShapeCell;
use plith_core::shapes::REST;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

fn monitor(width: i32, height: i32, scale: f64) -> MonitorInfo {
    MonitorInfo {
        handle: 7,
        rect: RectPx {
            x: 0,
            y: 0,
            width,
            height,
        },
        work_top_px: 0,
        scale,
        hz: 240.0,
    }
}

fn start() -> Display {
    display_for(monitor(2560, 1440, 1.0), false)
}

fn recording() -> (Inner, Rc<RefCell<Vec<ShellEvent>>>) {
    let events = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&events);
    let inner = Inner::new(
        start(),
        Arc::new(ShapeCell::default()),
        Box::new(move |event| sink.borrow_mut().push(event)),
        Box::new(|_| {}),
    );
    (inner, events)
}

#[test]
fn next_display_equal_is_none() {
    assert_eq!(next_display(start(), monitor(2560, 1440, 1.0), false), None);
}

#[test]
fn next_display_new_rect_recentres() {
    let wide = monitor(3840, 2160, 1.5);
    let next = next_display(start(), wide, false).expect("a new display");
    assert_eq!(next.layout, SurfaceLayout::new(wide.rect, 0, 1.5));
    assert_eq!(next.monitor, 7);
    assert!(!next.remote);
}

#[test]
fn next_display_remote_flip_is_a_change() {
    let next = next_display(start(), monitor(2560, 1440, 1.0), true).expect("a new display");
    assert!(next.remote);
}

#[test]
fn relayout_message_burst_posts_once() {
    let (inner, _) = recording();
    let posts = Cell::new(0);
    for _ in 0..5 {
        request(&inner, || {
            posts.set(posts.get() + 1);
            true
        });
    }
    assert_eq!(posts.get(), 1);
}

#[test]
fn a_failed_post_allows_the_next_request() {
    let (inner, _) = recording();
    let posts = Cell::new(0);
    for _ in 0..2 {
        request(&inner, || {
            posts.set(posts.get() + 1);
            false
        });
    }
    assert_eq!(posts.get(), 2);
}

#[test]
fn relayout_stores_and_emits_the_new_display() {
    let (inner, events) = recording();
    let wide = monitor(3840, 2160, 1.5);
    relayout_to(&inner, wide, false);
    let expected = display_for(wide, false);
    assert_eq!(inner.display.get(), expected);
    assert_eq!(inner.layout(), expected.layout);
    assert_eq!(*events.borrow(), vec![ShellEvent::DisplayChanged(expected)]);
}

#[test]
fn relayout_to_the_same_display_emits_nothing() {
    let (inner, events) = recording();
    relayout_to(&inner, monitor(2560, 1440, 1.0), false);
    assert!(events.borrow().is_empty());
}

#[test]
fn relayout_clears_applied_region() {
    let (inner, _) = recording();
    // The resting shape is what `apply_latest` covers with no frame posted: as the cached
    // entry it would skip the region at the new scale.
    inner.applied.set(Some(REST));
    relayout_to(&inner, monitor(3840, 2160, 1.5), false);
    assert_eq!(inner.stats.get().count, 1, "the region was rebuilt");
}

#[test]
fn relayout_clears_the_pending_request() {
    let (inner, _) = recording();
    request(&inner, || true);
    relayout_to(&inner, monitor(2560, 1440, 1.0), false);
    let posts = Cell::new(0);
    request(&inner, || {
        posts.set(posts.get() + 1);
        true
    });
    assert_eq!(posts.get(), 1);
}
