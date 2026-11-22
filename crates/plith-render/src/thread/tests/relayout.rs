//! A relayout reaches the loop beside the mailbox: at rest, mid-motion, suppressed, with a new
//! clock, and when the rebuild fails.

use super::super::{Fatal, Relayout, RelayoutHandle};
use super::expanded;
use super::harness::*;
use plith_core::clock_choice::ClockKind;
use plith_core::display::Display;
use plith_core::launch::ClockRequest;
use plith_core::layout::{RectPx, SurfaceLayout};
use plith_core::scene::{Scene, SceneMailbox, Suppression};
use plith_core::shapes::REST;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::thread;

fn wide() -> SurfaceLayout {
    let monitor = RectPx {
        x: 0,
        y: 0,
        width: 2560,
        height: 1440,
    };
    SurfaceLayout::new(monitor, 0, 1.5)
}

/// Asks the loop for `layout`, keeping the clock.
fn relayout(run: &Run, layout: SurfaceLayout) {
    run.shared.request_relayout(Relayout {
        layout,
        hz: 240.0,
        order: None,
    });
    let _ = run.mailbox.nudge();
}

fn wait_relayouts(run: &Run, n: usize) {
    let deadline = std::time::Instant::now() + BOUND;
    while lock(&run.relayouts).len() < n {
        assert!(
            std::time::Instant::now() < deadline,
            "expected {n} relayouts"
        );
        thread::sleep(std::time::Duration::from_millis(1));
    }
}

#[test]
fn relayout_at_rest_repaints_once_with_new_origin() {
    let run = start(Setup::default());
    run.wait_frames(1);
    relayout(&run, wide());
    run.wait_frames(2);
    run.assert_parked();
    let frames = run.frames();
    assert_eq!(frames.len(), 2, "one repaint for the relayout");
    assert_eq!(frames[1].origin, wide().island_origin(REST));
    assert_eq!(*lock(&run.relayouts), vec![wide()]);
    assert_eq!(run.harness.total(), 0, "no clock wait at rest");
    let _ = run.stop();
}

#[test]
fn relayout_mid_motion_keeps_animating() {
    let run = start(Setup::default());
    run.wait_frames(1);
    run.mailbox.publish(expanded(0));
    run.wait_calls(10);
    relayout(&run, wide());
    let report = run.report();
    assert!(report.frames > 10, "{report}");
    assert!(run.fatal.try_recv().is_err());
    let last = run.frames().pop().expect("frames");
    assert_eq!(last.origin, wide().island_origin(last.shape));
    let _ = run.stop();
}

#[test]
fn relayout_while_suppressed_paints_nothing() {
    let run = start(Setup::default());
    run.wait_frames(1);
    run.mailbox.publish(Scene {
        suppression: Suppression {
            locked: true,
            ..Suppression::default()
        },
        ..Scene::default()
    });
    relayout(&run, wide());
    wait_relayouts(&run, 1);
    run.assert_parked();
    assert_eq!(run.frames().len(), 1, "no paint while suppressed");
    let _ = run.stop();
}

#[test]
fn relayout_with_new_hz_reopens_clock() {
    let run = start(Setup::default());
    run.wait_frames(1);
    run.mailbox.publish(expanded(0));
    let _ = run.report();
    assert_eq!(run.opens.load(Ordering::SeqCst), 1);
    run.shared.request_relayout(Relayout {
        layout: wide(),
        hz: 60.0,
        order: Some(KINDS.to_vec()),
    });
    let _ = run.mailbox.nudge();
    wait_relayouts(&run, 1);
    run.mailbox.publish(Scene::default());
    let _ = run.report();
    assert_eq!(
        run.opens.load(Ordering::SeqCst),
        2,
        "the clock was reopened"
    );
    let notes: Vec<String> = run.notes.try_iter().collect();
    assert!(
        notes
            .iter()
            .any(|n| n.starts_with("render relayout:") && n.contains("hz=60.00")),
        "{notes:?}"
    );
    let _ = run.stop();
}

#[test]
fn relayout_failure_falls_back_to_warp() {
    let run = start(Setup {
        relayout_fails: true,
        ..Setup::default()
    });
    run.wait_frames(1);
    relayout(&run, wide());
    run.wait_frames(2);
    assert_eq!(*lock(&run.recovers), vec![true]);
    assert!(run.fatal.try_recv().is_err());
    let _ = run.stop();
}

#[test]
fn relayout_failure_reports_fatal() {
    let run = start(Setup {
        relayout_fails: true,
        recover_fails: true,
        ..Setup::default()
    });
    run.wait_frames(1);
    relayout(&run, wide());
    let fatal = run.fatal.recv_timeout(BOUND).expect("a fatal report");
    assert!(
        matches!(&fatal, Fatal::Error(text) if text.contains("relayout")),
        "{fatal:?}"
    );
    let _ = run.stop();
}

#[test]
fn a_later_request_keeps_an_earlier_clock_change() {
    let shared = super::super::LoopShared::new();
    shared.request_relayout(Relayout {
        layout: wide(),
        hz: 60.0,
        order: Some(vec![ClockKind::Timer]),
    });
    shared.request_relayout(Relayout {
        layout: layout(),
        hz: 240.0,
        order: None,
    });
    let taken = shared.take_relayout().expect("a pending relayout");
    assert_eq!(taken.layout, layout());
    assert_eq!(taken.order, Some(vec![ClockKind::Timer]));
    assert!((taken.hz - 60.0).abs() < f64::EPSILON);
    assert!(shared.take_relayout().is_none());
}

fn handle(start: Display) -> (RelayoutHandle, Arc<super::super::LoopShared>) {
    let shared = Arc::new(super::super::LoopShared::new());
    let handle = RelayoutHandle {
        mailbox: Arc::new(SceneMailbox::new(Scene::default(), Arc::default())),
        shared: Arc::clone(&shared),
        clock: ClockRequest::Auto,
        last: Arc::new(Mutex::new(start)),
    };
    (handle, shared)
}

#[test]
fn relayout_handle_orders_the_clock_only_on_a_change() {
    let start = Display {
        layout: layout(),
        monitor: 3,
        hz: 240.0,
        remote: false,
    };
    let (handle, shared) = handle(start);
    handle.relayout(Display {
        layout: wide(),
        ..start
    });
    assert_eq!(shared.take_relayout().expect("pending").order, None);
    handle.relayout(Display {
        layout: wide(),
        remote: true,
        ..start
    });
    let taken = shared.take_relayout().expect("pending");
    assert_eq!(taken.order, Some(vec![ClockKind::Timer]));
    assert_eq!(shared.clock_target(), (3, 240.0));
}
