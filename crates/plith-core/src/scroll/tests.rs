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
