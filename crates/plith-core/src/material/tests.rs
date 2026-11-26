// Exact comparisons are intended: clamped endpoints and untouched copies are bit-identical.
#![allow(clippy::float_cmp)]

use super::*;

const EPS: f32 = 1e-4;

fn expanded(now: f64) -> MaterialMotion {
    let mut m = MaterialMotion::new();
    m.expand(now, 0.0);
    m
}

#[test]
fn layers_at_rest_collapsed_and_open() {
    let m = MaterialMotion::new();
    assert_eq!(m.layers(0.0, 0.0), MaterialLayers::COLLAPSED);
    let m = expanded(0.0);
    assert_eq!(m.layers(5000.0, 1.0), MaterialLayers::OPEN);
}

#[test]
fn open_at_rest_is_settled_and_collapses_from_nothing() {
    let mut m = MaterialMotion::open();
    assert!(m.is_settled(0.0, 1.0));
    assert_eq!(m.layers(0.0, 1.0), MaterialLayers::OPEN);
    m.expand(10.0, 1.0);
    assert!(
        m.is_settled(10.0, 1.0),
        "already open: expand replays nothing"
    );
    m.collapse(10.0);
    assert_eq!(m.layers(10.0, 1.0).ripples, [0.0; 3]);
    assert!(m.is_settled(10.0 + 1000.0, 0.0));
}

#[test]
fn ripples_press_in_order_at_listed_times() {
    let m = expanded(1000.0);
    for (k, start) in RIPPLE_START_MS.iter().enumerate() {
        let l = m.layers(1000.0 + start + PRESS_MS, 1.0);
        assert!((l.ripples[k] - 1.0).abs() < EPS, "ripple {k} not pressed");
        if k < 2 {
            // The listed starts are 88.8 ms apart and a press lasts 90 ms,
            // so the next one has barely begun (about 1.3 % of its press).
            assert!(l.ripples[k + 1] < 0.02, "ripple {} started early", k + 1);
        }
    }
    let before_first = m.layers(1000.0 + RIPPLE_START_MS[0] - 1.0, 1.0);
    assert_eq!(before_first.ripples, [0.0; 3]);
}

#[test]
fn ripples_gone_after_hold_and_fade() {
    let m = expanded(0.0);
    let end = RIPPLE_START_MS[2] + PRESS_MS + HOLD_MS;
    let held = m.layers(end - 1.0, 1.0);
    assert!(held.ripples.iter().all(|r| *r > 0.99));
    let mid = m.layers(end + FADE_MS / 2.0, 1.0);
    assert!(mid.ripples.iter().all(|r| (*r - 0.5).abs() < 0.01));
    assert_eq!(m.layers(end + FADE_MS, 1.0).ripples, [0.0; 3]);
}

#[test]
fn reveal_full_at_progress_055() {
    let m = expanded(0.0);
    assert!((m.layers(0.0, 0.55).reveal - 1.0).abs() < 1e-9);
    assert!((m.layers(0.0, 0.275).reveal - 0.5).abs() < 1e-9);
    assert_eq!(m.layers(0.0, 0.9).reveal, 1.0);
}

#[test]
fn disc_lift_between_030_and_060() {
    let m = expanded(0.0);
    assert_eq!(m.layers(0.0, 0.30).disc_lift, 0.0);
    assert!((m.layers(0.0, 0.45).disc_lift - 20.0).abs() < 1e-9);
    assert!((m.layers(0.0, 0.60).disc_lift - 40.0).abs() < 1e-9);
    assert_eq!(m.layers(0.0, 1.0).disc_lift, 40.0);
    // Pill is gone at 0.30, disc shows while lifting, then it is out.
    assert_eq!(m.layers(0.0, 0.30).pill, 0.0);
    assert!((m.layers(0.0, 0.15).pill - 0.5).abs() < EPS);
    assert!((m.layers(0.0, 0.30).disc - 1.0).abs() < EPS);
    assert_eq!(m.layers(0.0, 0.60).disc, 0.0);
}

#[test]
fn collapse_mid_expand_starts_from_current() {
    let mut m = expanded(0.0);
    let now = RIPPLE_START_MS[1] + 40.0;
    let before = m.layers(now, 0.5);
    assert!(before.ripples[0] > 0.0 && before.ripples[1] > 0.0);
    m.collapse(now);
    assert_eq!(m.layers(now, 0.5), before);
    // Ripple 3 leaves first (it is 0 here), ripple 1 last.
    let later = m.layers(now + RISE_STEP_MS, 0.4);
    assert_eq!(later.ripples[0], before.ripples[0]);
    let end = m.layers(now + 2.0 * RISE_STEP_MS + FADE_MS, 0.0);
    assert!(end.ripples.iter().all(|r| *r < 1e-6));
}

#[test]
fn collapse_ripples_leave_in_order_3_2_1() {
    let mut m = expanded(0.0);
    let now = 450.0;
    m.collapse(now);
    let l = m.layers(now + RISE_STEP_MS, 0.5);
    assert!(l.ripples[2] < l.ripples[1] && l.ripples[1] <= l.ripples[0]);
}

#[test]
fn reopen_replays_only_below_ten_percent() {
    // Reveal 0.5 (progress 0.275): no replay.
    let mut m = expanded(0.0);
    m.collapse(1000.0);
    m.expand(1100.0, 0.275);
    assert_eq!(
        m.layers(1100.0 + RIPPLE_START_MS[0] + PRESS_MS, 0.5)
            .ripples,
        [0.0; 3]
    );
    // Reveal 0.05 (progress 0.0275): replays.
    let mut m = expanded(0.0);
    m.collapse(1000.0);
    m.expand(1300.0, 0.0275);
    let l = m.layers(1300.0 + RIPPLE_START_MS[0] + PRESS_MS, 0.3);
    assert!((l.ripples[0] - 1.0).abs() < EPS);
}

#[test]
fn page_switch_does_not_replay() {
    let m = MaterialMotion::new();
    for p in [1.0, 0.8, 1.0] {
        let l = m.layers(10_000.0, p);
        assert_eq!(l.ripples, [0.0; 3]);
    }
    let m = expanded(0.0);
    for (t, p) in [(5000.0, 1.0), (5010.0, 0.8), (5020.0, 1.0)] {
        assert_eq!(m.layers(t, p).ripples, [0.0; 3]);
    }
}

#[test]
fn settled_only_when_ripples_done_and_progress_at_end() {
    let m = expanded(0.0);
    let done = RIPPLE_START_MS[2] + PRESS_MS + HOLD_MS + FADE_MS;
    assert!(!m.is_settled(0.0, 1.0));
    assert!(!m.is_settled(done - 1.0, 1.0));
    assert!(!m.is_settled(done, 0.5));
    assert!(m.is_settled(done, 1.0));
    let mut c = expanded(0.0);
    c.collapse(done);
    assert!(!c.is_settled(done + 10.0, 0.0));
    let after = done + 2.0 * RISE_STEP_MS + FADE_MS;
    assert!(!c.is_settled(after, 0.4));
    assert!(c.is_settled(after, 0.0));
    assert!(MaterialMotion::new().is_settled(0.0, 0.0));
}

#[test]
fn morph_progress_clamps() {
    assert_eq!(morph_progress(30.0, 30.0, 130.0), 0.0);
    assert_eq!(morph_progress(80.0, 30.0, 130.0), 0.5);
    assert_eq!(morph_progress(500.0, 30.0, 130.0), 1.0);
    assert_eq!(morph_progress(10.0, 30.0, 130.0), 0.0);
    assert_eq!(morph_progress(50.0, 30.0, 30.0), 1.0);
}

#[test]
fn reopen_mid_collapse_continues_ripples() {
    // 0.5 progress: no replay branch; 0.02: replay branch.
    for progress in [0.5, 0.02] {
        let mut m = expanded(0.0);
        let now = RIPPLE_START_MS[1] + 40.0;
        m.collapse(now);
        let at = now + 100.0;
        let before = m.layers(at, progress);
        assert!(before.ripples[0] > 0.0, "need a visible ripple");
        m.expand(at, progress);
        assert_eq!(
            m.layers(at, progress),
            before,
            "jump at progress {progress}"
        );
    }
}

#[test]
fn replay_never_dips_below_current() {
    let mut m = expanded(0.0);
    let now = 500.0;
    m.collapse(now);
    let at = now + 100.0;
    let start = m.layers(at, 0.02).ripples;
    m.expand(at, 0.02);
    for dt in 0..40 {
        let l = m.layers(at + f64::from(dt) * 5.0, 0.02);
        for (k, (r, s)) in l.ripples.iter().zip(start).enumerate() {
            assert!(*r >= s - 1e-6, "ripple {k} dipped");
        }
    }
}
