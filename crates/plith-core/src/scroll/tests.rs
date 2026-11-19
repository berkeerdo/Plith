#![allow(clippy::float_cmp)]

use super::*;

#[test]
fn offset_rests_at_zero_after_run() {
    let run = scroll_run_s(80.0);
    assert_eq!(scroll_offset(run, 80.0), 0.0);
    assert_eq!(scroll_offset(run + 5.0, 80.0), 0.0);
    assert_eq!(scroll_offset(0.0, 80.0), 0.0);
    assert_eq!(scroll_offset(-1.0, 80.0), 0.0);
    assert_eq!(scroll_offset(SCROLL_HOLD_S * 0.5, 80.0), 0.0);
}

#[test]
fn offset_reaches_minus_overflow() {
    let far = SCROLL_HOLD_S + 80.0 / SCROLL_SPEED;
    assert!((scroll_offset(far + 0.1, 80.0) + 80.0).abs() < 1e-9);
    let mid = SCROLL_HOLD_S + 1.0;
    assert!((scroll_offset(mid, 80.0) + SCROLL_SPEED).abs() < 1e-9);
    let back = far + SCROLL_HOLD_S + 1.0;
    assert!((scroll_offset(back, 80.0) + 40.0).abs() < 1e-9);
}

#[test]
fn no_overflow_never_moves() {
    assert_eq!(scroll_run_s(0.0), 0.0);
    assert_eq!(scroll_run_s(-3.0), 0.0);
    assert_eq!(scroll_offset(1.0, 0.0), 0.0);
    assert_eq!(scroll_offset(1.0, -5.0), 0.0);
}

#[test]
fn run_is_two_holds_and_two_travels() {
    assert!((scroll_run_s(80.0) - (1.6 + 4.0)).abs() < 1e-9);
}

#[test]
fn run_offset_follows_the_measured_overflow_when_shorter() {
    let run = scroll_run_s(80.0);
    let far = SCROLL_HOLD_S + 50.0 / SCROLL_SPEED;
    assert!((scroll_offset_in_run(far + 0.1, run, 50.0) + 50.0).abs() < 1e-9);
    // Home early, then at rest until the run ends.
    assert_eq!(scroll_offset_in_run(run - 0.01, run, 50.0), 0.0);
}

#[test]
fn run_offset_is_capped_by_the_run_when_the_measure_is_longer() {
    let run = scroll_run_s(80.0);
    let far = SCROLL_HOLD_S + 80.0 / SCROLL_SPEED;
    assert!((scroll_offset_in_run(far + 0.1, run, 120.0) + 80.0).abs() < 1e-9);
    // Continuous into rest at the run's end.
    assert!(scroll_offset_in_run(run - 0.001, run, 120.0).abs() < 0.05);
}

#[test]
fn run_offset_rests_outside_the_run() {
    assert_eq!(scroll_offset_in_run(-1.0, 5.0, 80.0), 0.0);
    assert_eq!(scroll_offset_in_run(5.0, 5.0, 80.0), 0.0);
    assert_eq!(scroll_offset_in_run(9.0, 5.0, 80.0), 0.0);
    assert_eq!(scroll_offset_in_run(2.0, f64::NAN, 80.0), 0.0);
    assert_eq!(scroll_offset_in_run(2.0, 1.0, 80.0), 0.0);
}
