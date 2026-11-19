use super::*;
use crate::draw::{DisplayList, LiveNode, LiveRail, Node, Rect};
use crate::scene::Suppression;
use std::sync::Arc;
use std::time::Duration;

const NOW: SystemTime = SystemTime::UNIX_EPOCH;

const RECT: Rect = Rect {
    x: 0.0,
    y: 0.0,
    w: 10.0,
    h: 10.0,
};

fn live_list() -> Arc<DisplayList> {
    Arc::new(DisplayList {
        nodes: vec![
            Node::Fill {
                rect: RECT,
                radius: 0.0,
                ink: crate::draw::Ink::Floor,
            },
            Node::Live(LiveNode::Meter {
                rect: RECT,
                rail: LiveRail::Strip,
                index: 0,
            }),
        ],
        size: (10.0, 10.0),
    })
}

fn content_live_on(page: usize) -> PageContent {
    let mut content = PageContent::empty();
    content.pages[page] = live_list();
    content
}

fn expanded(page: usize) -> Scene {
    Scene {
        expanded: true,
        page,
        ..Scene::default()
    }
}

#[test]
fn divisor_at_60_120_144_240() {
    assert_eq!(ambient_divisor(60.0), 1);
    assert_eq!(ambient_divisor(120.0), 2);
    assert_eq!(ambient_divisor(144.0), 2);
    assert_eq!(ambient_divisor(240.0), 4);
}

#[test]
fn divisor_is_at_least_one() {
    assert_eq!(ambient_divisor(30.0), 1);
    assert_eq!(ambient_divisor(0.0), 1);
    assert_eq!(ambient_divisor(f64::NAN), 1);
}

#[test]
fn wants_ambient_needs_expanded_unsuppressed_and_a_live_node_on_the_page() {
    let live = content_live_on(1);
    assert!(wants_ambient(&expanded(1), &live, NOW));
    assert!(
        !wants_ambient(&expanded(0), &live, NOW),
        "page 0 has no live node"
    );
    let collapsed = Scene {
        page: 1,
        ..Scene::default()
    };
    assert!(!wants_ambient(&collapsed, &live, NOW));
    let suppressed = Scene {
        suppression: Suppression {
            fullscreen: true,
            ..Suppression::default()
        },
        ..expanded(1)
    };
    assert!(!wants_ambient(&suppressed, &live, NOW));
    assert!(!wants_ambient(&expanded(1), &PageContent::empty(), NOW));
}

#[test]
fn wants_ambient_is_false_for_an_out_of_range_page() {
    let scene = expanded(9);
    assert!(!wants_ambient(&scene, &content_live_on(3), NOW));
}

#[test]
fn live_values_default_is_empty_at_the_epoch() {
    let values = LiveValues::default();
    assert!(values.strips.is_empty());
    assert!(values.buses.is_empty());
    assert!(values.traces.is_empty());
    assert_eq!(values.now, SystemTime::UNIX_EPOCH);
    assert_eq!(TraceSamples(vec![0.5]).0, vec![0.5]);
}

#[test]
fn report_display_is_one_line() {
    let report = AmbientReport {
        target: AmbientTarget::Page(2),
        secs: 12.34,
        frames: 740,
        hitches: 1,
        worst_gap_ms: 25.004,
        draw_p99_ms: 0.456,
    };
    assert_eq!(
        report.to_string(),
        "ambient: page 2 12.3 s, 740 frames, 1 hitches, worst 25.00 ms, draw p99 0.46 ms"
    );
}

#[test]
fn stats_of_a_steady_run_have_no_hitches() {
    let hz = 240.0;
    let interval = 4.0 / hz;
    let mut stats = AmbientStats::new(1, hz);
    for i in 0..100_u32 {
        stats.record_frame(f64::from(i) * interval, 1.0);
    }
    let report = stats.finish();
    assert_eq!(report.target, AmbientTarget::Page(1));
    assert_eq!(report.frames, 100);
    assert_eq!(report.hitches, 0);
    assert!((report.worst_gap_ms - interval * 1000.0).abs() < 1e-9);
    assert!((report.secs - 100.0 * interval).abs() < 1e-9);
    assert!((report.draw_p99_ms - 1.0).abs() < 1e-9);
}

#[test]
fn a_gap_over_one_and_a_half_intervals_is_a_hitch() {
    let hz = 60.0;
    let interval = 1.0 / hz;
    let mut stats = AmbientStats::new(0, hz);
    stats.record_frame(0.0, 0.5);
    stats.record_frame(interval, 0.5);
    stats.record_frame(interval * 2.4, 0.5);
    stats.record_frame(interval * 4.4, 0.5);
    let report = stats.finish();
    assert_eq!(report.hitches, 1);
    assert!((report.worst_gap_ms - 2.0 * interval * 1000.0).abs() < 1e-9);
}

#[test]
fn draw_p99_is_the_nearest_rank_to_two_decimals() {
    let mut stats = AmbientStats::new(0, 240.0);
    for i in 0..200_u32 {
        let draw = if i < 197 { 0.3 } else { 2.5 };
        stats.record_frame(f64::from(i) / 60.0, draw);
    }
    // Rank ceil(0.99 * 200) = 198: the first of the three slow frames.
    assert!((stats.finish().draw_p99_ms - 2.5).abs() < 1e-9);
}

#[test]
fn an_empty_run_reports_zeros() {
    let report = AmbientStats::new(3, 240.0).finish();
    assert_eq!(report.frames, 0);
    assert_eq!(report.hitches, 0);
    assert!(report.secs.abs() < f64::EPSILON);
    assert!(report.worst_gap_ms.abs() < f64::EPSILON);
    assert!(report.draw_p99_ms.abs() < f64::EPSILON);
}

#[test]
fn peak_is_the_louder_side_and_missing_is_zero() {
    let live = LiveValues {
        strips: vec![[0.2, 0.6]],
        buses: vec![[0.0, 0.0], [0.9, 0.1]],
        ..LiveValues::default()
    };
    assert!((live.peak(LiveRail::Strip, 0) - 0.6).abs() < 1e-6);
    assert!((live.peak(LiveRail::Bus, 1) - 0.9).abs() < 1e-6);
    assert!(live.peak(LiveRail::Strip, 4).abs() < 1e-6);
    assert!(live.peak(LiveRail::Bus, 9).abs() < 1e-6);
}

#[test]
fn trace_samples_by_index_or_empty() {
    let live = LiveValues {
        traces: vec![TraceSamples(vec![0.1, 0.2])],
        ..LiveValues::default()
    };
    assert_eq!(live.trace(0), &[0.1, 0.2]);
    assert!(live.trace(3).is_empty());
}

fn live_page() -> Arc<DisplayList> {
    Arc::new(DisplayList {
        nodes: vec![Node::Live(LiveNode::Meter {
            rect: RECT,
            rail: LiveRail::Strip,
            index: 0,
        })],
        size: (10.0, 10.0),
    })
}

#[test]
fn shows_live_needs_a_visible_layer_with_a_live_node() {
    use crate::page::{PageLayer, PageLayers};
    let mut content = PageContent::empty();
    content.pages[2] = live_page();
    let layer = |page: usize, opacity: f64| PageLayer {
        page,
        offset: 0.0,
        opacity,
    };
    // Outgoing live page, incoming plain page: still live.
    let turning = PageLayers::of(&[layer(2, 0.5), layer(1, 0.5)]);
    assert!(shows_live(&content, &turning, 1.0));
    assert!(!shows_live(&content, &turning, 0.0), "hidden content");
    let gone = PageLayers::of(&[layer(2, 0.0), layer(1, 1.0)]);
    assert!(!shows_live(&content, &gone, 1.0), "faded out layer");
    assert!(!shows_live(
        &content,
        &PageLayers::of(&[layer(0, 1.0)]),
        1.0
    ));
}

fn pill_scroll(start: SystemTime, run_s: f64) -> PageContent {
    let mut content = PageContent::empty();
    content.pill = Arc::new(DisplayList {
        nodes: vec![Node::Live(LiveNode::Scroll {
            rect: RECT,
            text: Arc::from("a long title"),
            style: crate::draw::TextStyle::Value,
            ink: crate::draw::Ink::Muted,
            start,
            run_s,
        })],
        size: (10.0, 10.0),
    });
    content
}

#[test]
fn ambient_for_pill_scroll_only_until_run_ends() {
    let start = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
    let content = pill_scroll(start, 5.0);
    let collapsed = Scene::default();
    assert!(wants_ambient(&collapsed, &content, start));
    let late = start + Duration::from_secs(4);
    assert!(wants_ambient(&collapsed, &content, late));
    let done = start + Duration::from_secs(5);
    assert!(!wants_ambient(&collapsed, &content, done));
    let suppressed = Scene {
        suppression: Suppression {
            fullscreen: true,
            ..Suppression::default()
        },
        ..Scene::default()
    };
    assert!(!wants_ambient(&suppressed, &content, start));
    assert!(
        !wants_ambient(&expanded(1), &content, start),
        "an expanded island ignores the pill"
    );
}

#[test]
fn ambient_none_for_pill_without_scroll() {
    let content = PageContent::empty();
    assert!(!wants_ambient(&Scene::default(), &content, NOW));
}

#[test]
fn ambient_none_when_clock_is_before_start() {
    let start = SystemTime::UNIX_EPOCH + Duration::from_secs(7200);
    let content = pill_scroll(start, 5.0);
    let collapsed = Scene::default();
    let hour_before = start - Duration::from_secs(3600);
    assert!(
        !wants_ambient(&collapsed, &content, hour_before),
        "a clock that moved back is not a running scroll"
    );
    let just_before = start - Duration::from_millis(1);
    assert!(!wants_ambient(&collapsed, &content, just_before));
    assert!(wants_ambient(&collapsed, &content, start), "the start runs");
}

#[test]
fn pill_report_names_the_pill() {
    let report = AmbientReport {
        target: AmbientTarget::Pill,
        secs: 1.0,
        frames: 240,
        hitches: 0,
        worst_gap_ms: 4.17,
        draw_p99_ms: 0.3,
    };
    assert_eq!(
        report.to_string(),
        "ambient: pill 1.0 s, 240 frames, 0 hitches, worst 4.17 ms, draw p99 0.30 ms"
    );
}

#[test]
fn pill_stats_paint_every_tick() {
    let hz = 240.0;
    let mut stats = AmbientStats::pill(hz);
    assert_eq!(stats.divisor(), 1);
    assert_eq!(
        AmbientStats::new(2, hz).divisor(),
        4,
        "pages keep the divisor"
    );
    for i in 0..240_u32 {
        stats.record_frame(f64::from(i) / hz, 0.5);
    }
    let report = stats.finish();
    assert_eq!(report.target, AmbientTarget::Pill);
    assert_eq!(report.hitches, 0, "one period apart is no hitch");
    assert!((report.secs - 1.0).abs() < 1e-9, "secs {}", report.secs);
}
