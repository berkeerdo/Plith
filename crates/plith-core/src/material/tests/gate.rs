//! The content gate: in Halka the page content of an open waits until the ripples have pressed
//! and faded, so no frame shows ripples and page text together (Berke, 2026-11-26).

use super::*;
use crate::island::IslandMotion;
use crate::shapes::{PAGES, REST};
use crate::tuning::BRISK;

/// One frame at 240 Hz, in ms.
const FRAME_MS: f64 = 1000.0 / 240.0;
/// The latest the gate may open in a BRISK expand from rest to page 0. The floor is the press
/// (progress 0.85 at about 152 ms), the hold and the fade: 312.5 ms measured (170 before the
/// slower ripples of 2026-11-27).
const GATE_BY_MS: f64 = 320.0;

/// A BRISK expand from rest to page 0 at 240 Hz: (time in ms, morph progress) per frame.
fn brisk_curve() -> Vec<(f64, f64)> {
    let line = MorphLine::open(REST.height, PAGES[0].height);
    let mut island = IslandMotion::new(REST, BRISK);
    island.retarget(PAGES[0]);
    (0..240)
        .map(|k| {
            let at = (
                f64::from(k) * FRAME_MS,
                line.progress(island.shape().height),
            );
            island.step(FRAME_MS / 1000.0);
            at
        })
        .collect()
}

fn max(ripples: [f32; 3]) -> f32 {
    ripples.into_iter().fold(0.0, f32::max)
}

#[test]
fn content_waits_for_the_ripples_in_a_brisk_expand() {
    let mut m = expanded(0.0);
    assert!(!m.content_gate(), "closed before the first press");
    let mut pressed = false;
    let mut opened_at = None;
    for (now, progress) in brisk_curve() {
        m.advance(now, progress);
        let ripples = m.layers(now, progress).ripples;
        pressed |= ripples.iter().all(|r| *r >= 1.0);
        if max(ripples) > 0.0 {
            assert!(!m.content_gate(), "gate open with ripples at {now:.1} ms");
        }
        match opened_at {
            None if m.content_gate() => opened_at = Some(now),
            None => {}
            Some(_) => assert!(m.content_gate(), "gate closed again at {now:.1} ms"),
        }
        if pressed && max(ripples) == 0.0 {
            assert!(
                m.content_gate(),
                "ripples gone but gate shut at {now:.1} ms"
            );
        }
    }
    assert!(pressed, "the three ripples pressed");
    let opened_at = opened_at.expect("the gate opened");
    assert!(opened_at <= GATE_BY_MS, "gate opened at {opened_at:.1} ms");
}

#[test]
fn the_gate_opens_when_hold_and_fade_end() {
    let mut m = pressed_at(100.0);
    at(&mut m, 100.0 + HOLD_MS + FADE_MS - 1.0, 1.0);
    assert!(!m.content_gate(), "still fading");
    let l = at(&mut m, 100.0 + HOLD_MS + FADE_MS, 1.0);
    assert_eq!(l.ripples, [0.0; 3]);
    assert!(m.content_gate());
    assert!(m.is_settled(100.0 + HOLD_MS + FADE_MS, 1.0));
}

#[test]
fn the_gate_is_open_at_rest_and_through_a_page_switch() {
    assert!(MaterialMotion::open().content_gate());
    let mut m = MaterialMotion::open();
    m.expand(10.0, 1.0);
    m.advance(10.0, 1.0);
    assert!(m.content_gate(), "a page switch keeps the content");
}

#[test]
fn a_replay_closes_the_gate_until_its_ripples_are_gone() {
    let mut m = MaterialMotion::open();
    m.collapse(0.0);
    m.expand(1000.0, 0.02);
    m.advance(1000.0, 0.02);
    assert!(!m.content_gate(), "a replay waits for its ripples");
    at(&mut m, 1100.0, ALL_PRESSED_AT);
    assert!(!m.content_gate());
    at(&mut m, 1100.0 + HOLD_MS + FADE_MS, 1.0);
    assert!(m.content_gate());
}

#[test]
fn a_reopen_without_replay_opens_once_the_collapse_ripples_are_gone() {
    // From open at rest: no ripples to wait for.
    let mut m = MaterialMotion::open();
    m.collapse(0.0);
    m.expand(100.0, 0.5);
    assert!(m.content_gate(), "nothing to wait for");
    // From a collapse that cut an expand short: wait for its ripples to leave.
    let mut m = pressed_at(100.0);
    m.collapse(120.0);
    m.expand(130.0, 0.5);
    let l = at(&mut m, 140.0, 0.6);
    assert!(max(l.ripples) > 0.0 && !m.content_gate());
    let done = 120.0 + 2.0 * RISE_STEP_MS + RISE_MS;
    assert_eq!(at(&mut m, done, 1.0).ripples, [0.0; 3]);
    assert!(m.content_gate());
}

#[test]
fn the_collapse_keeps_its_timing() {
    assert!((RISE_STEP_MS - 60.0).abs() < 1e-12);
    assert!((RISE_MS - 180.0).abs() < 1e-12);
    let mut m = pressed_at(100.0);
    m.collapse(100.0);
    let l = at(&mut m, 100.0 + RISE_MS / 2.0, 0.5);
    assert!(
        (l.ripples[2] - 0.5).abs() < 0.01,
        "ripple 3 half way: {l:?}"
    );
}

#[test]
fn the_idle_look_leaves_with_the_pill() {
    // A plain theme: the idle look is what the content leaves, as always.
    for content in [0.0, 0.25, 1.0] {
        assert_eq!(idle_look_alpha(content, None), 1.0 - content);
    }
    // Halka: never above the pill, so it is gone once the pill is, whatever the content.
    let mut m = expanded(0.0);
    for (now, progress) in brisk_curve() {
        m.advance(now, progress);
        let pill = m.layers(now, progress).pill;
        let idle = idle_look_alpha(0.0, Some(pill));
        assert!(idle <= pill, "idle {idle} above pill {pill} at {now:.1} ms");
        if pill == 0.0 {
            assert_eq!(idle, 0.0, "idle look over the fabric at {now:.1} ms");
        }
    }
    assert_eq!(
        idle_look_alpha(0.0, Some(1.0)),
        1.0,
        "collapsed: the look shows"
    );
    let handing = idle_look_alpha(0.6, Some(1.0));
    assert!((handing - 0.4).abs() < 1e-6, "the content still hands over");
}
