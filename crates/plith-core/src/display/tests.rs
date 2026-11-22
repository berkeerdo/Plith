use super::*;
use crate::layout::{RectPx, SurfaceLayout};

fn display(scale: f64, hz: f64, remote: bool) -> Display {
    Display {
        layout: SurfaceLayout {
            surface: RectPx {
                x: 1280,
                y: 0,
                width: 960,
                height: 400,
            },
            scale,
        },
        monitor: 7,
        hz,
        remote,
    }
}

#[test]
fn same_monitor_new_scale_keeps_clock() {
    let old = display(1.0, 240.0, false);
    let next = display(1.5, 240.0, false);
    assert!(!old.clock_changed(&next));
}

#[test]
fn remote_flip_changes_clock() {
    let old = display(1.0, 240.0, false);
    let next = display(1.0, 240.0, true);
    assert!(old.clock_changed(&next));
}

#[test]
fn hz_change_changes_clock() {
    let old = display(1.0, 240.0, false);
    let next = display(1.0, 60.0, false);
    assert!(old.clock_changed(&next));
}

#[test]
fn new_monitor_changes_clock() {
    let old = display(1.0, 240.0, false);
    let next = Display {
        monitor: 8,
        ..old
    };
    assert!(old.clock_changed(&next));
}

#[test]
fn log_line_format() {
    assert_eq!(
        display(1.5, 240.0, false).log_line(),
        "relayout: rect=1280,0 960x400 scale=1.50 hz=240.00 remote=false"
    );
}
