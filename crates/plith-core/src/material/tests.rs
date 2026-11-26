// Exact comparisons are intended: clamped endpoints and untouched copies are bit-identical.
#![allow(clippy::float_cmp)]

use super::*;

const EPS: f32 = 1e-4;

fn expanded(now: f64) -> MaterialMotion {
    let mut m = MaterialMotion::new();
    m.expand(now, 0.0);
    m
}

/// Advances `m` to `progress` at `now` and returns the layers there.
fn at(m: &mut MaterialMotion, now: f64, progress: f64) -> MaterialLayers {
    m.advance(now, progress);
    m.layers(now, progress)
}

/// An expand driven to full press at `t` (progress 0.6), the moment the hold starts.
fn pressed_at(t: f64) -> MaterialMotion {
    let mut m = expanded(0.0);
    at(&mut m, t / 2.0, 0.3);
    at(&mut m, t, ALL_PRESSED_AT);
    m
}

#[test]
fn layers_at_rest_collapsed_and_open() {
    let m = MaterialMotion::new();
    assert_eq!(m.layers(0.0, 0.0), MaterialLayers::COLLAPSED);
    let mut m = expanded(0.0);
    at(&mut m, 100.0, 1.0);
    assert_eq!(at(&mut m, 5000.0, 1.0), MaterialLayers::OPEN);
}

#[test]
fn open_at_rest_is_settled_and_collapses_from_nothing() {
    let mut m = MaterialMotion::open();
    assert!(m.is_settled(0.0, 1.0));
    assert_eq!(at(&mut m, 0.0, 1.0), MaterialLayers::OPEN);
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
fn all_pressed_where_the_last_press_ends() {
    assert!((ALL_PRESSED_AT - 0.60).abs() < 1e-12);
    assert_eq!(RIPPLE_FROM, [0.15, 0.30, 0.45]);
}

#[test]
fn ripples_press_in_order_by_progress() {
    let mut m = expanded(0.0);
    assert_eq!(at(&mut m, 1.0, RIPPLE_FROM[0]).ripples, [0.0; 3]);
    for (k, from) in RIPPLE_FROM.iter().enumerate() {
        let half = at(&mut m, 2.0, from + PRESS_SPAN / 2.0);
        assert!((half.ripples[k] - 0.5).abs() < EPS, "ripple {k} half way");
        let l = at(&mut m, 3.0, from + PRESS_SPAN);
        assert!((l.ripples[k] - 1.0).abs() < EPS, "ripple {k} not pressed");
        if k < 2 {
            assert_eq!(l.ripples[k + 1], 0.0, "ripple {} started early", k + 1);
        }
    }
}

#[test]
fn a_press_never_comes_back_out() {
    let mut m = expanded(0.0);
    let before = at(&mut m, 50.0, 0.5).ripples;
    for p in [0.4, 0.2, 0.0] {
        assert_eq!(at(&mut m, 60.0, p).ripples, before, "dip to {p}");
    }
}

#[test]
fn ripples_gone_after_hold_and_fade() {
    let mut m = pressed_at(100.0);
    let held = at(&mut m, 100.0 + HOLD_MS - 1.0, 1.0);
    assert!(held.ripples.iter().all(|r| *r == 1.0));
    let mid = at(&mut m, 100.0 + HOLD_MS + FADE_MS / 2.0, 1.0);
    assert!(mid.ripples.iter().all(|r| (*r - 0.5).abs() < 0.01));
    let end = at(&mut m, 100.0 + HOLD_MS + FADE_MS, 1.0);
    assert_eq!(end.ripples, [0.0; 3]);
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
    let now = 80.0;
    let before = at(&mut m, now, 0.4);
    assert!(before.ripples[0] == 1.0 && before.ripples[1] > 0.0);
    m.collapse(now);
    assert_eq!(at(&mut m, now, 0.4), before);
    // Ripple 3 leaves first (it is 0 here), ripple 1 last.
    let later = at(&mut m, now + RISE_STEP_MS, 0.3);
    assert_eq!(later.ripples[0], before.ripples[0]);
    let end = at(&mut m, now + 2.0 * RISE_STEP_MS + FADE_MS, 0.0);
    assert!(end.ripples.iter().all(|r| *r < 1e-6));
}

#[test]
fn collapse_ripples_leave_in_order_3_2_1() {
    let mut m = pressed_at(100.0);
    let now = 150.0;
    m.collapse(now);
    let l = at(&mut m, now + RISE_STEP_MS, 0.5);
    assert!(l.ripples[2] < l.ripples[1] && l.ripples[1] <= l.ripples[0]);
}

#[test]
fn reopen_replays_only_below_ten_percent() {
    // Reveal 0.5 (progress 0.275): no replay, the ripples only keep fading.
    let mut m = pressed_at(100.0);
    m.collapse(1000.0);
    m.expand(1100.0, 0.275);
    assert_eq!(at(&mut m, 1200.0, 1.0).ripples, [0.0; 3]);
    // Reveal 0.05 (progress 0.0275): replays as the progress grows again.
    let mut m = pressed_at(100.0);
    m.collapse(1000.0);
    m.expand(1300.0, 0.0275);
    let l = at(&mut m, 1350.0, ALL_PRESSED_AT);
    assert!(l.ripples.iter().all(|r| (*r - 1.0).abs() < EPS));
}

#[test]
fn page_switch_does_not_replay() {
    let mut m = pressed_at(100.0);
    let done = 100.0 + HOLD_MS + FADE_MS;
    for (t, p) in [(done, 1.0), (done + 10.0, 0.4), (done + 20.0, 1.0)] {
        m.expand(t, p);
        assert_eq!(at(&mut m, t, p).ripples, [0.0; 3]);
    }
}

#[test]
fn settled_only_when_ripples_done_and_progress_at_end() {
    let mut m = expanded(0.0);
    assert!(!m.is_settled(10_000.0, 1.0), "never pressed: not done");
    at(&mut m, 100.0, 1.0);
    let done = 100.0 + HOLD_MS + FADE_MS;
    assert!(!m.is_settled(done - 1.0, 1.0));
    assert!(!m.is_settled(done, 0.5));
    assert!(m.is_settled(done, 1.0));
    let mut c = pressed_at(100.0);
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
        let mut m = pressed_at(100.0);
        let now = 150.0;
        m.collapse(now);
        let t = now + 100.0;
        let before = at(&mut m, t, progress);
        assert!(before.ripples[0] > 0.0, "need a visible ripple");
        m.expand(t, progress);
        assert_eq!(
            at(&mut m, t, progress),
            before,
            "jump at progress {progress}"
        );
    }
}

#[test]
fn replay_never_dips_below_current() {
    let mut m = pressed_at(100.0);
    let now = 150.0;
    m.collapse(now);
    let t = now + 100.0;
    let start = at(&mut m, t, 0.02).ripples;
    m.expand(t, 0.02);
    for step in 0..40 {
        let p = 0.02 + f64::from(step) * 0.02;
        let l = at(&mut m, t + f64::from(step) * 5.0, p);
        for (k, (r, s)) in l.ripples.iter().zip(start).enumerate() {
            assert!(*r >= s - 1e-6, "ripple {k} dipped");
        }
    }
}

#[test]
fn the_open_line_is_morph_progress() {
    let line = MorphLine::open(32.0, 300.0);
    for h in [3.0, 32.0, 100.0, 300.0, 330.0] {
        assert_eq!(line.progress(h), morph_progress(h, 32.0, 300.0));
    }
}

#[test]
fn a_retarget_keeps_the_progress_where_it_is() {
    // Reopen at 120 DIP of a collapse from 300 toward a 170 page: no jump, 1 at the target.
    let collapse = MorphLine::open(32.0, 300.0).collapse(300.0, 32.0);
    let now = collapse.progress(120.0);
    let reopen = collapse.toward(120.0, 170.0);
    assert!((reopen.progress(120.0) - now).abs() < 1e-12);
    assert_eq!(reopen.progress(170.0), 1.0);
    // Toward a taller page from the same point.
    let taller = collapse.toward(120.0, 400.0);
    assert!((taller.progress(120.0) - now).abs() < 1e-12);
    assert_eq!(taller.progress(400.0), 1.0);
}

#[test]
fn an_open_island_stays_at_one_through_a_page_switch() {
    let open = MorphLine::open(32.0, 300.0);
    let page = open.toward(300.0, 170.0);
    for h in [300.0, 240.0, 170.0, 165.0] {
        assert_eq!(page.progress(h), 1.0, "at {h}");
    }
}

#[test]
fn a_collapse_runs_from_the_drawn_progress_to_zero() {
    // Collapse mid-expand at 100 DIP of an open toward 300.
    let open = MorphLine::open(32.0, 300.0);
    let p = open.progress(100.0);
    let down = open.collapse(100.0, 32.0);
    assert!((down.progress(100.0) - p).abs() < 1e-12);
    assert_eq!(down.progress(32.0), 0.0);
    assert_eq!(down.progress(3.0), 0.0);
    // At rest height the line is flat at 0.
    assert_eq!(
        MorphLine::open(32.0, 300.0)
            .collapse(32.0, 32.0)
            .progress(32.0),
        0.0
    );
}
