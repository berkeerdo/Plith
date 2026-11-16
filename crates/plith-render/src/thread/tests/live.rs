//! Content that changes without motion and the ambient tick.

use super::expanded;
use super::harness::*;
use plith_core::ambient::{AmbientReport, LiveValues};
use plith_core::draw::{
    Align, DisplayList, Ink, LiveNode, LiveRail, Node, PageContent, Rect, TextStyle, Trim,
};
use plith_core::motion::{MotionKind, MotionReport};
use plith_core::scene::{Scene, Suppression};
use plith_core::shapes::{PAGES, REST};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

/// The clock call at which the ambient tests publish their next scene; well after the expand.
const AT: u64 = 1000;

const RECT: Rect = Rect {
    x: 8.0,
    y: 8.0,
    w: 100.0,
    h: 12.0,
};

fn page_zero(node: Node) -> Arc<PageContent> {
    let mut content = PageContent::empty();
    content.pages[0] = Arc::new(DisplayList {
        nodes: vec![node],
        size: (300.0, 100.0),
    });
    Arc::new(content)
}

fn text_content(text: &str) -> Arc<PageContent> {
    page_zero(Node::Text {
        rect: RECT,
        text: Arc::from(text),
        style: TextStyle::Title,
        ink: Ink::Ink,
        align: Align::Start,
        trim: Trim::End,
    })
}

fn live_content() -> Arc<PageContent> {
    page_zero(Node::Live(LiveNode::Meter {
        rect: RECT,
        rail: LiveRail::Strip,
        index: 0,
    }))
}

fn is_ambient(live: &LiveValues) -> bool {
    *live != LiveValues::default()
}

/// Live content, expand, ambient until clock call `AT`, then `next` is published.
fn ambient_then(next: Scene) -> (Run, MotionReport, AmbientReport) {
    let run = start(Setup {
        content: Some(live_content()),
        script: vec![(AT, next)],
        ..Setup::default()
    });
    run.wait_frames(1);
    run.mailbox.publish(expanded(0));
    let expand = run.report();
    assert_eq!(expand.kind, MotionKind::Expand);
    assert!(
        (expand.frames as u64) < AT - 40,
        "the expand ends well before the script"
    );
    let ambient = run.ambient_report();
    (run, expand, ambient)
}

#[test]
fn content_change_at_rest_paints_once() {
    let run = start(Setup::default());
    run.wait_frames(1);
    run.mailbox.publish(expanded(0));
    let _ = run.report();
    run.assert_parked();
    let before = run.frames().len();
    let wakes = run.wakes();
    let calls = run.harness.total();
    let content = text_content("Track title");
    let generation = run
        .mailbox
        .publish_content(Arc::clone(&content))
        .expect("new content bumps");
    run.wait_frames(before + 1);
    run.assert_parked();
    let frames = run.frames();
    assert_eq!(frames.len(), before + 1, "exactly one frame");
    assert_eq!(frames[before - 1].content_gen, 0);
    let last = &frames[before];
    assert_eq!(last.content, content);
    assert_eq!(last.content_gen, generation);
    assert_eq!(last.shape, PAGES[0]);
    assert_eq!(last.live, LiveValues::default());
    assert_eq!(run.wakes(), wakes + 1, "one mailbox wake, no clock wait");
    assert_eq!(run.harness.total(), calls);
    assert!(run.reports.recv_timeout(QUIET).is_err(), "no motion line");
    assert!(run.ambient.try_recv().is_err(), "no ambient line");
    let _ = run.stop();
}

#[test]
fn content_change_mid_motion_keeps_motion() {
    let baseline = start(Setup::default());
    baseline.wait_frames(1);
    baseline.mailbox.publish(expanded(0));
    let plain = baseline.report();
    baseline.assert_parked();
    let plain_frames = baseline.frames();
    let _ = baseline.stop();

    let content = text_content("Next track");
    let run = start(Setup {
        content_script: vec![(30, Arc::clone(&content))],
        ..Setup::default()
    });
    run.wait_frames(1);
    run.mailbox.publish(expanded(0));
    let report = run.report();
    run.assert_parked();
    assert_eq!(report.kind, MotionKind::Expand);
    assert_eq!(report.retargets, 0, "content is no retarget");
    assert_eq!(report.frames, plain.frames);
    let frames = run.frames();
    let shapes: Vec<_> = frames.iter().map(|f| f.shape).collect();
    let plain_shapes: Vec<_> = plain_frames.iter().map(|f| f.shape).collect();
    assert_eq!(shapes, plain_shapes, "the springs keep their velocity");
    // Frame k of the motion is painted after clock call k; the content arrives in call 30.
    assert_eq!(frames[29].content, Arc::new(PageContent::empty()));
    assert_eq!(frames[30].content, content);
    assert_eq!(frames[30].content_gen, 2);
    assert!(run.reports.recv_timeout(QUIET).is_err(), "one motion line");
    assert!(run.ambient.try_recv().is_err());
    let _ = run.stop();
}

#[test]
fn ambient_runs_only_when_expanded_with_live_nodes() {
    // Collapsed with live content: parked from the start.
    let collapsed = start(Setup {
        content: Some(live_content()),
        ..Setup::default()
    });
    collapsed.wait_frames(1);
    collapsed.assert_parked();
    assert_eq!(collapsed.wakes(), 0);
    let _ = collapsed.stop();

    // Expanded without live nodes: parked after the motion.
    let plain = start(Setup {
        content: Some(text_content("Title")),
        ..Setup::default()
    });
    plain.wait_frames(1);
    plain.mailbox.publish(expanded(0));
    let _ = plain.report();
    plain.assert_parked();
    assert!(plain.ambient.try_recv().is_err());
    assert_eq!(plain.samples.load(Ordering::SeqCst), 0);
    let _ = plain.stop();

    // Expanded with live nodes: ticks until a suppression parks it, with no motion line.
    let suppressed = Scene {
        suppression: Suppression {
            fullscreen: true,
            ..Suppression::default()
        },
        ..expanded(0)
    };
    let (run, expand, ambient) = ambient_then(suppressed);
    assert!(ambient.frames > 100, "ambient frames {}", ambient.frames);
    run.assert_parked();
    assert_eq!(run.harness.total(), AT);
    let frames = run.frames();
    assert!(
        frames[1 + expand.frames..]
            .iter()
            .all(|f| is_ambient(&f.live)),
        "every frame after the expand is an ambient frame"
    );
    assert!(
        frames[..=expand.frames]
            .iter()
            .all(|f| !is_ambient(&f.live))
    );
    assert!(run.reports.recv_timeout(QUIET).is_err(), "no motion line");
    let _ = run.stop();
}

#[test]
fn ambient_paints_every_fourth_tick_at_240() {
    let (run, expand, ambient) = ambient_then(Scene::default());
    let _ = run.report();
    let frames = run.frames();
    let calls = run.calls_at();
    let ticks: Vec<u64> = frames
        .iter()
        .zip(&calls)
        .filter(|(f, _)| is_ambient(&f.live))
        .map(|(_, c)| *c)
        .collect();
    let first = expand.frames as u64;
    assert_eq!(ticks.first(), Some(&(first + 4)));
    assert!(ticks.windows(2).all(|w| w[1] - w[0] == 4), "every 4th tick");
    // The tick that sees the collapse paints nothing.
    assert_eq!(ticks.len() as u64, (AT - 1 - first) / 4);
    assert_eq!(ambient.frames, ticks.len());
    assert_eq!(run.samples.load(Ordering::SeqCst), ticks.len() as u64);
    let levels: Vec<f32> = frames
        .iter()
        .filter(|f| is_ambient(&f.live))
        .map(|f| f.live.strips[0][0])
        .collect();
    assert!(
        levels.windows(2).all(|w| w[1] > w[0]),
        "one sample per frame"
    );
    let _ = run.stop();
}

#[test]
fn collapse_stops_ambient_and_reports() {
    let (run, expand, ambient) = ambient_then(Scene::default());
    let collapse = run.report();
    assert_eq!(collapse.kind, MotionKind::Collapse);
    assert_eq!(collapse.retargets, 0);
    assert_eq!(collapse.frames as u64, run.harness.total() - AT);
    run.assert_parked();
    assert_eq!(run.frames().last().map(|f| f.shape), Some(REST));
    assert_eq!(ambient.page, 0);
    assert_eq!(ambient.hitches, 0);
    let interval_ms = 4.0 * PERIOD * 1000.0;
    assert!((ambient.worst_gap_ms - interval_ms).abs() < 1e-6);
    assert!((ambient.draw_p99_ms - 1.0).abs() < 1e-9);
    let secs = f64::from(u32::try_from(ambient.frames).unwrap()) * 4.0 * PERIOD;
    assert!((ambient.secs - secs).abs() < 1e-9, "secs {}", ambient.secs);
    let line = ambient.to_string();
    assert!(line.starts_with("ambient: page 0 "), "{line}");
    assert!(
        line.ends_with(" frames, 0 hitches, worst 16.67 ms, draw p99 1.00 ms"),
        "{line}"
    );
    assert!(run.ambient.recv_timeout(QUIET).is_err(), "one ambient line");
    assert!(run.reports.try_recv().is_err(), "expand and collapse only");
    assert!(expand.frames > 0);
    let _ = run.stop();
}

#[test]
fn idle_after_collapse_has_no_wakes() {
    let (run, _, _) = ambient_then(Scene::default());
    let _ = run.report();
    run.assert_parked();
    let wakes = run.wakes();
    let calls = run.harness.total();
    let frames = run.frames().len();
    thread::sleep(Duration::from_millis(200));
    assert_eq!(run.wakes(), wakes, "no wake while idle");
    assert_eq!(run.harness.total(), calls);
    assert_eq!(run.frames().len(), frames);
    assert_eq!(wakes, 1 + calls, "one mailbox wake, then only clock waits");
    let _ = run.stop();
}
