//! Page bitmaps warmed outside the measured frames (around a motion, at rest and between
//! ambient ticks), and the present decision for a content-only snapshot.

use super::expanded;
use super::harness::*;
use plith_core::draw::{
    Align, DisplayList, Ink, LiveNode, LiveRail, Node, PageContent, Rect, TextStyle, Trim,
};
use plith_core::motion::MotionKind;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

const RECT: Rect = Rect {
    x: 8.0,
    y: 8.0,
    w: 100.0,
    h: 12.0,
};

fn text_list(text: &str) -> Arc<DisplayList> {
    Arc::new(DisplayList {
        nodes: vec![Node::Text {
            rect: RECT,
            text: Arc::from(text),
            style: TextStyle::Title,
            ink: Ink::Ink,
            align: Align::Start,
            trim: Trim::End,
        }],
        size: (300.0, 100.0),
    })
}

/// Five text pages (the welcome view last) and a text pill.
fn pages() -> Arc<PageContent> {
    let mut content = PageContent::empty();
    for (page, text) in ["Audio", "Media", "Shelf", "System", "Welcome"]
        .iter()
        .enumerate()
    {
        content.pages[page] = text_list(text);
    }
    content.pill = text_list("Pill");
    Arc::new(content)
}

/// `base` with page `page` replaced by `text`.
fn with_page(base: &PageContent, page: usize, text: &str) -> Arc<PageContent> {
    let mut content = base.clone();
    content.pages[page] = text_list(text);
    Arc::new(content)
}

/// Waits until the loop has taken at least `n` wakes.
fn wait_wakes(run: &Run, n: u64) {
    let deadline = Instant::now() + BOUND;
    while run.wakes() < n {
        assert!(Instant::now() < deadline, "expected {n} wakes");
        thread::sleep(Duration::from_millis(1));
    }
}

/// Waits until `n` prewarm calls were made.
fn wait_prewarms(run: &Run, n: usize) {
    let deadline = Instant::now() + BOUND;
    while run.prewarms().len() < n {
        assert!(Instant::now() < deadline, "expected {n} prewarm calls");
        thread::sleep(Duration::from_millis(1));
    }
}

/// An expanded, settled loop on page 0 with every page warmed; the notes so far are drained.
fn open_at_rest() -> Run {
    let run = start(Setup {
        content: Some(pages()),
        ..Setup::default()
    });
    run.wait_frames(1);
    run.mailbox.publish(expanded(0));
    let _ = run.report();
    run.assert_parked();
    let _ = run.prewarm_notes();
    run
}

#[test]
fn collapsed_content_rasters_nothing_and_presents_only_the_pill() {
    let base = pages();
    let run = start(Setup {
        content: Some(Arc::clone(&base)),
        ..Setup::default()
    });
    run.wait_frames(1);
    run.assert_parked();
    let wakes = run.wakes();
    let hidden = with_page(&base, 1, "Next track");
    run.mailbox.publish_content(Arc::clone(&hidden));
    wait_wakes(&run, wakes + 1);
    run.assert_parked();
    assert_eq!(
        run.frames().len(),
        1,
        "a hidden page change presents nothing"
    );
    assert!(run.prewarms().is_empty(), "collapsed: no raster at all");
    let mut pill = (*hidden).clone();
    pill.pill = text_list("Muted");
    run.mailbox.publish_content(Arc::new(pill));
    run.wait_frames(2);
    run.assert_parked();
    assert_eq!(run.frames().len(), 2, "the shown pill changed: one present");
    assert!(run.prewarms().is_empty());
    assert!(run.prewarm_notes().is_empty());
    let _ = run.stop();
}

#[test]
fn an_aimed_expand_prewarms_start_and_target_before_the_first_clock_wait() {
    let run = start(Setup {
        content: Some(pages()),
        ..Setup::default()
    });
    run.wait_frames(1);
    run.assert_parked();
    let calls = run.harness.total();
    run.mailbox.publish(expanded(2));
    let report = run.report();
    assert_eq!(report.kind, MotionKind::Expand);
    run.assert_parked();
    let warms = run.prewarms();
    assert_eq!(warms.len(), 2, "{warms:?}");
    assert_eq!(
        warms[0],
        (vec![0, 2], calls),
        "exactly the start and target pages, before the first clock wait"
    );
    assert_eq!(
        warms[1].0,
        vec![1, 3, 4, 5],
        "the other pages after it settled"
    );
    assert!(warms[1].1 >= calls + report.frames as u64);
    assert_eq!(
        run.prewarm_notes(),
        vec![
            "prewarm: pages 0,2 0.75 ms (aim)".to_string(),
            "prewarm: pages 1,3,4,5 0.75 ms (rest)".to_string(),
        ]
    );
    let _ = run.stop();
}

#[test]
fn content_at_rest_prewarms_and_presents_only_a_shown_change() {
    let run = open_at_rest();
    let before = run.frames().len();
    let warmed = run.prewarms().len();
    let hidden = with_page(&run.mailbox.latest().content, 3, "Brightness 40");
    run.mailbox.publish_content(Arc::clone(&hidden));
    wait_prewarms(&run, warmed + 1);
    run.assert_parked();
    assert_eq!(run.prewarms()[warmed].0, vec![3]);
    assert_eq!(run.frames().len(), before, "page 3 is not on screen");
    assert_eq!(
        run.prewarm_notes(),
        vec!["prewarm: pages 3 0.75 ms (rest)".to_string()]
    );
    run.mailbox.publish_content(with_page(&hidden, 0, "Routes"));
    run.wait_frames(before + 1);
    run.assert_parked();
    assert_eq!(run.prewarms()[warmed + 1].0, vec![0]);
    assert_eq!(run.frames().len(), before + 1, "the shown page changed");
    let _ = run.stop();
}

#[test]
fn a_turn_to_warm_pages_prewarms_nothing() {
    let run = open_at_rest();
    let warmed = run.prewarms().len();
    run.mailbox.publish(expanded(1));
    let report = run.report();
    assert_eq!(report.kind, MotionKind::Page);
    run.assert_parked();
    assert_eq!(run.prewarms().len(), warmed, "every page was already warm");
    assert!(run.prewarm_notes().is_empty());
    let _ = run.stop();
}

#[test]
fn ambient_content_prewarms_between_ticks() {
    const AT: u64 = 1000;
    let mut live = (*pages()).clone();
    live.pages[0] = Arc::new(DisplayList {
        nodes: vec![Node::Live(LiveNode::Meter {
            rect: RECT,
            rail: LiveRail::Strip,
            index: 0,
        })],
        size: (300.0, 100.0),
    });
    let live = Arc::new(live);
    let run = start(Setup {
        content: Some(Arc::clone(&live)),
        content_script: vec![(AT, with_page(&live, 1, "Next track"))],
        ..Setup::default()
    });
    run.wait_frames(1);
    run.mailbox.publish(expanded(0));
    let _ = run.report();
    wait_prewarms(&run, 3);
    let warms = run.prewarms();
    assert_eq!(warms[2].0, vec![1]);
    assert!(
        warms[2].1 >= AT,
        "taken by the ambient tick after call {AT}"
    );
    let notes = run.prewarm_notes();
    assert_eq!(
        notes.last().map(String::as_str),
        Some("prewarm: pages 1 0.75 ms (ambient)")
    );
    let _ = run.stop();
}

#[test]
fn opening_warms_the_welcome_page_too() {
    let run = start(Setup {
        content: Some(pages()),
        ..Setup::default()
    });
    run.wait_frames(1);
    run.assert_parked();
    let welcome = plith_core::pages::PageId::Welcome.index();
    run.mailbox.publish(expanded(0));
    let _ = run.report();
    run.assert_parked();
    let warmed: Vec<usize> = run.prewarms().into_iter().flat_map(|w| w.0).collect();
    assert!(warmed.contains(&welcome), "{warmed:?}");
    let _ = run.stop();
}
