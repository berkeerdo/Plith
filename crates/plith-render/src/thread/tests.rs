mod harness;
mod live;
mod palette;
mod prewarm;
mod relayout;

use super::{Fatal, RenderHandle};
use harness::*;
use plith_core::clock_choice::ClockKind;
use plith_core::geometry::IslandShape;
use plith_core::island::IslandMotion;
use plith_core::motion::MotionKind;
use plith_core::scene::{Scene, Suppression};
use plith_core::shapes::{PAGES, REST};
use plith_core::tuning::R1;
use std::thread;
use std::time::{Duration, Instant};

fn expanded(page: usize) -> Scene {
    Scene {
        expanded: true,
        page,
        suppression: Suppression::default(),
        ..Scene::default()
    }
}

fn one_period_from(from: IslandShape, to: IslandShape) -> IslandShape {
    let mut motion = IslandMotion::new(from, R1);
    motion.retarget(to);
    motion.step(PERIOD);
    motion.shape()
}

#[test]
fn collapse_report_has_a_visible_duration_shorter_than_the_full_one() {
    let run = start(Setup::default());
    run.wait_frames(1);
    run.mailbox.publish(expanded(0));
    let _ = run.report();
    run.mailbox.publish(Scene::default());
    let report = run.report();
    assert_eq!(report.kind, MotionKind::Collapse);
    assert!(report.visible_ms > 100.0, "visible {}", report.visible_ms);
    assert!(
        report.visible_ms < report.duration_ms - 100.0,
        "visible {} vs full {}",
        report.visible_ms,
        report.duration_ms
    );
    assert!(report.to_string().contains(", visible "), "{report}");
    let _ = run.stop();
}

#[test]
fn parks_at_rest_with_no_clock_waits() {
    let run = start(Setup::default());
    run.wait_frames(1);
    thread::sleep(Duration::from_millis(200));
    assert_eq!(run.harness.total(), 0);
    assert_eq!(run.wakes(), 0);
    let frames = run.frames();
    assert_eq!(frames.len(), 1, "exactly one rest frame at start");
    assert_eq!(frames[0].shape, REST);
    assert!(frames[0].content_opacity.abs() < f32::EPSILON);
    assert!(
        run.reports.try_recv().is_err(),
        "the start frame is no motion"
    );
    let _ = run.stop();
}

#[test]
fn suppressed_scene_parks_even_mid_motion() {
    let suppressed = Scene {
        suppression: Suppression {
            fullscreen: true,
            ..Suppression::default()
        },
        ..expanded(0)
    };
    let run = start(Setup {
        script: vec![(20, suppressed)],
        ..Setup::default()
    });
    run.wait_frames(1);
    run.mailbox.publish(expanded(0));
    run.wait_calls(20);
    run.assert_parked();
    assert_eq!(
        run.harness.total(),
        20,
        "no clock wait after the suppression"
    );
    assert!(
        run.reports.try_recv().is_err(),
        "the motion report is dropped"
    );
    assert_eq!(
        run.frames().len(),
        1 + 19,
        "the suppressed frame is not painted"
    );
    assert_eq!(run.wakes(), 1 + 20);
    let _ = run.stop();
}

#[test]
fn unsuppress_paints_one_frame_without_clock_waits() {
    let suppressed = Scene {
        suppression: Suppression {
            locked: true,
            ..Suppression::default()
        },
        ..expanded(0)
    };
    let run = start(Setup {
        script: vec![(20, suppressed)],
        ..Setup::default()
    });
    run.wait_frames(1);
    run.mailbox.publish(expanded(0));
    run.wait_calls(20);
    run.assert_parked();
    run.mailbox.publish(expanded(0));
    run.wait_frames(21);
    run.assert_parked();
    let frames = run.frames();
    assert_eq!(frames.len(), 21, "exactly one frame for the snapped target");
    assert_eq!(
        frames[20].shape, PAGES[0],
        "shows the target with no catch-up"
    );
    let shapes = lock(&run.shapes).clone();
    assert_eq!(
        shapes[20].previous, PAGES[0],
        "a snap forgets the frame before it: previous equals the shape"
    );
    assert_eq!(run.harness.total(), 20);
    assert_eq!(run.wakes(), 1 + 20 + 1);
    assert!(run.reports.try_recv().is_err());
    let _ = run.stop();
}

#[test]
fn one_expand_produces_one_motion_report() {
    let run = start(Setup::default());
    run.wait_frames(1);
    run.mailbox.publish(expanded(0));
    let report = run.report();
    assert_eq!(report.kind, MotionKind::Expand);
    assert!(report.frames > 100, "frames {}", report.frames);
    assert_eq!(report.retargets, 0);
    assert_eq!(report.clock, "dcomp");
    assert_eq!(report.hitches, 0);
    assert!(
        (report.first_wait_ms - PERIOD * 1000.0).abs() < 1e-6,
        "first wait {}",
        report.first_wait_ms
    );
    run.assert_parked();
    let calls = run.harness.total();
    assert_eq!(u64::try_from(report.frames).ok(), Some(calls));
    assert_eq!(run.wakes(), 1 + calls);
    let frames = run.frames();
    assert_eq!(frames.len(), 1 + report.frames);
    let last = frames.last().expect("frames");
    assert_eq!(last.shape, PAGES[0]);
    assert!((last.content_opacity - 1.0).abs() < f32::EPSILON);
    assert!(run.reports.recv_timeout(QUIET).is_err(), "only one report");
    let _ = run.stop();
}

#[test]
fn a_settled_collapse_ends_with_hidden_content() {
    let run = start(Setup::default());
    run.wait_frames(1);
    run.mailbox.publish(expanded(0));
    let _ = run.report();
    run.assert_parked();
    let opened = run.frames();
    let open_last = opened.last().expect("frames");
    assert!((open_last.content_opacity - 1.0).abs() < f32::EPSILON);
    run.mailbox.publish(Scene::default());
    let _ = run.report();
    run.assert_parked();
    let frames = run.frames();
    let last = frames.last().expect("frames");
    assert_eq!(last.shape, REST);
    assert!(
        last.content_opacity.abs() < f32::EPSILON,
        "content opacity {}",
        last.content_opacity
    );
    assert!(
        frames[opened.len()..]
            .iter()
            .all(|f| f.content_opacity < 1.0),
        "the collapse fades the content out"
    );
    let _ = run.stop();
}

#[test]
fn retarget_mid_motion_is_one_report_with_retargets_1() {
    let run = start(Setup {
        script: vec![(30, expanded(1))],
        ..Setup::default()
    });
    run.wait_frames(1);
    run.mailbox.publish(expanded(0));
    let report = run.report();
    assert_eq!(report.kind, MotionKind::Expand);
    assert_eq!(report.retargets, 1);
    run.assert_parked();
    assert!(run.reports.recv_timeout(QUIET).is_err(), "only one report");
    assert_eq!(run.frames().last().map(|f| f.shape), Some(PAGES[1]));
    let _ = run.stop();
}

#[test]
fn first_frame_after_rest_advances_one_period() {
    let run = start(Setup::default());
    run.wait_frames(1);
    run.mailbox.publish(expanded(0));
    let expand = run.report();
    let frames = run.frames();
    assert_eq!(frames[1].shape, one_period_from(REST, PAGES[0]));
    assert_eq!(frames[1].origin, layout().island_origin(frames[1].shape));
    run.assert_parked();
    // A rest gap of 12 periods: without a timeline and stamper reset the first step would be 13.
    run.harness.advance(12.0 * PERIOD);
    run.mailbox.publish(Scene::default());
    let collapse = run.report();
    assert_eq!(collapse.kind, MotionKind::Collapse);
    let first = &run.frames()[1 + expand.frames];
    assert_eq!(first.shape, one_period_from(PAGES[0], REST));
    let _ = run.stop();
}

#[test]
fn on_shape_is_called_every_painted_frame() {
    let run = start(Setup::default());
    run.wait_frames(1);
    run.mailbox.publish(expanded(0));
    let _ = run.report();
    run.assert_parked();
    let frames = run.frames();
    let shapes = lock(&run.shapes).clone();
    assert_eq!(shapes.len(), frames.len());
    for (i, (shape, frame)) in shapes.iter().zip(&frames).enumerate() {
        assert_eq!(shape.shape, frame.shape, "frame {i}");
        let expected_previous = if i == 0 {
            frame.shape
        } else {
            frames[i - 1].shape
        };
        assert_eq!(shape.previous, expected_previous, "previous of frame {i}");
        assert_eq!(shape.generation, u64::try_from(i + 1).ok().unwrap_or(0));
    }
    let _ = run.stop();
}

#[test]
fn three_fast_returns_demote_the_clock() {
    let run = start(Setup {
        instant: vec![ClockKind::Dcomp],
        ..Setup::default()
    });
    run.wait_frames(1);
    run.mailbox.publish(expanded(0));
    let report = run.report();
    assert_eq!(report.clock, "dxgi");
    // The first wait has no previous tick to measure against; the next three are fast.
    assert_eq!(run.harness.calls(ClockKind::Dcomp), 4);
    assert_eq!(run.harness.calls(ClockKind::Timer), 0);
    run.assert_parked();
    assert_eq!(run.wakes(), 1 + run.harness.total());
    // The next motion starts on the preferred clock again and demotes it again.
    run.mailbox.publish(Scene::default());
    assert_eq!(run.report().clock, "dxgi");
    assert_eq!(run.harness.calls(ClockKind::Dcomp), 8);
    let _ = run.stop();
}

#[test]
fn short_waits_between_regular_ticks_never_demote_the_clock() {
    let run = start(Setup {
        lead: 0.9 * PERIOD,
        ..Setup::default()
    });
    run.wait_frames(1);
    run.mailbox.publish(expanded(0));
    let report = run.report();
    assert_eq!(report.clock, "dcomp");
    assert_eq!(run.harness.calls(ClockKind::Dxgi), 0);
    let _ = run.stop();
}

#[test]
fn failed_paints_recreate_then_fall_back_to_warp() {
    let run = start(Setup {
        fail: 3,
        ..Setup::default()
    });
    let deadline = Instant::now() + BOUND;
    while lock(&run.recovers).is_empty() {
        assert!(Instant::now() < deadline, "the failed start frame recovers");
        thread::sleep(Duration::from_millis(1));
    }
    run.mailbox.publish(expanded(0));
    let report = run.report();
    // Start frame: fail, recover, repaint fails, recover. First motion frame: fail, WARP
    // fallback, repaint succeeds. Every clock wait ends in a painted frame.
    assert_eq!(*lock(&run.recovers), vec![false, false, true]);
    assert_eq!(
        report.frames,
        usize::try_from(run.harness.total()).unwrap_or(0)
    );
    assert_eq!(run.frames().len(), report.frames);
    let _ = run.stop();
}

#[test]
fn failed_recover_ends_the_thread_with_a_fatal_message() {
    let run = start(Setup {
        fail: 1,
        recover_fails: true,
        ..Setup::default()
    });
    let fatal = run.fatal.recv_timeout(BOUND).expect("a fatal message");
    assert!(matches!(fatal, Fatal::Error(ref text) if !text.is_empty()));
    run.done
        .recv_timeout(BOUND)
        .expect("the loop ends by itself");
    assert_eq!(run.harness.total(), 0);
}

#[test]
fn stop_mid_motion_ends_the_loop_without_a_report() {
    let run = start(Setup {
        delay: Some(Duration::from_millis(1)),
        ..Setup::default()
    });
    run.wait_frames(1);
    run.mailbox.publish(expanded(0));
    run.wait_calls(5);
    let reports = run.stop();
    assert!(
        reports.try_recv().is_err(),
        "a stopped motion is not reported"
    );
}

#[test]
fn a_failed_clock_is_retried_on_the_next_motion() {
    let run = start(Setup {
        fail_once: Some(ClockKind::Dcomp),
        ..Setup::default()
    });
    run.wait_frames(1);
    run.mailbox.publish(expanded(0));
    let expand = run.report();
    assert_eq!(expand.clock, "dxgi", "the failing wait demotes the motion");
    assert_eq!(run.harness.calls(ClockKind::Dcomp), 1);
    run.assert_parked();
    run.mailbox.publish(Scene::default());
    let collapse = run.report();
    assert_eq!(
        collapse.clock, "dcomp",
        "the next motion starts on the first source"
    );
    assert_eq!(
        run.harness.calls(ClockKind::Dcomp),
        1 + u64::try_from(collapse.frames).unwrap_or(0)
    );
    assert_eq!(run.wakes(), 2 + run.harness.total());
    let _ = run.stop();
}

#[test]
fn a_recovered_paint_repaints_the_same_frame() {
    let run = start(Setup {
        fail: 1,
        ..Setup::default()
    });
    run.wait_frames(1);
    run.assert_parked();
    assert_eq!(*lock(&run.recovers), vec![false]);
    let frames = run.frames();
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].shape, REST);
    let shapes = lock(&run.shapes).clone();
    assert_eq!(shapes.len(), 1, "on_shape for the repainted frame");
    assert_eq!(shapes[0].generation, 1);
    assert_eq!(run.harness.total(), 0, "no clock wait for the repaint");
    assert_eq!(run.wakes(), 0);
    let _ = run.stop();
}

#[test]
fn panic_in_paint_reaches_on_fatal() {
    let run = start(Setup {
        panic_at: Some(2),
        ..Setup::default()
    });
    run.wait_frames(1);
    run.mailbox.publish(expanded(0));
    let fatal = run.fatal.recv_timeout(BOUND).expect("a fatal message");
    assert!(
        matches!(fatal, Fatal::Panic(_)),
        "a panic is its own variant: {fatal:?}"
    );
    let message = fatal.message();
    assert!(message.contains("panicked"), "{message}");
    assert!(message.contains("fake paint blew up"), "{message}");
    run.done
        .recv_timeout(BOUND)
        .expect("the guarded loop returns instead of unwinding");
    run.join
        .join()
        .expect("the panic did not escape the thread");
}

#[test]
fn stop_and_join_is_idempotent() {
    let run = start(Setup::default());
    run.wait_frames(1);
    let Run {
        mailbox,
        shared,
        done,
        join,
        ..
    } = run;
    let config = super::RenderConfig {
        hwnd: 0,
        monitor: 0,
        layout: layout(),
        clock: plith_core::launch::ClockRequest::Auto,
        remote: false,
        hz: 240.0,
        tune: R1,
        levers: plith_core::levers::Levers::default(),
        palette: plith_core::theme::Palette::default(),
    };
    let handle = RenderHandle::new(&config, mailbox, shared, join);
    let other = handle.stop_handle();
    handle.stop_handle().stop_and_join();
    // The thread sends `done` before it exits, so a join that only requested the stop fails here.
    done.try_recv()
        .expect("the thread had ended when the first join returned");
    other.stop_and_join();
    handle.shutdown();
}
