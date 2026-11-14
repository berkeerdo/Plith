use super::*;
use crate::tuning::{GROW_W, OPACITY, RADIUS_GROW};

const H: f64 = 1.0 / 240.0;

/// Steps a spring one sub-step at a time; returns (peak, settle time in seconds).
fn run(spring: &mut Spring, limit_s: f64) -> (f64, f64) {
    let mut peak = f64::MIN;
    let mut t = 0.0;
    while t < limit_s {
        let settled = spring.step(H);
        t += H;
        peak = peak.max(spring.value());
        if settled {
            return (peak, t);
        }
    }
    (peak, f64::INFINITY)
}

#[test]
fn grow_overshoots_between_1_and_6_percent_and_settles_under_1_2_s() {
    let mut s = Spring::new(0.0, GROW_W);
    s.retarget(100.0, GROW_W);
    let (peak, settle) = run(&mut s, 5.0);
    let overshoot = (peak - 100.0) / 100.0;
    assert!((0.01..=0.06).contains(&overshoot), "overshoot {overshoot}");
    assert!(settle < 1.2, "settle {settle}");
}

#[test]
fn critical_spring_never_overshoots() {
    let mut s = Spring::new(0.0, RADIUS_GROW);
    s.retarget(100.0, RADIUS_GROW);
    let (peak, settle) = run(&mut s, 5.0);
    assert!(peak <= 100.0 + 1e-9, "peak {peak}");
    assert!(settle.is_finite());
}

#[test]
fn retarget_keeps_value_and_velocity() {
    let mut s = Spring::new(0.0, GROW_W);
    s.retarget(100.0, GROW_W);
    s.step(0.05);
    let (v, vel) = (s.value(), s.velocity());
    s.retarget(50.0, RADIUS_GROW);
    assert_bits(s.value(), v);
    assert_bits(s.velocity(), vel);
    assert_bits(s.target(), 50.0);
}

#[test]
fn settled_snaps_to_target() {
    let mut s = Spring::new(0.0, GROW_W);
    s.retarget(100.0, GROW_W);
    let (_, settle) = run(&mut s, 5.0);
    assert!(settle.is_finite());
    assert!(s.is_settled());
    assert_bits(s.value(), s.target());
    assert_bits(s.velocity(), 0.0);
}

#[test]
fn dt_is_clamped() {
    let mut a = Spring::new(0.0, GROW_W);
    a.retarget(100.0, GROW_W);
    let mut b = a.clone();
    a.step(5.0);
    b.step(0.1);
    assert!((a.value() - b.value()).abs() < 1e-9);
    assert!((a.velocity() - b.velocity()).abs() < 1e-9);
}

#[test]
fn leftover_time_is_carried_between_calls() {
    let mut a = Spring::new(0.0, GROW_W);
    a.retarget(100.0, GROW_W);
    let mut b = a.clone();
    for _ in 0..24 {
        a.step(H / 2.0);
    }
    b.step(12.0 * H);
    assert!((a.value() - b.value()).abs() < 1e-9);
}

#[test]
fn clamp_min_stops_negative_velocity() {
    let mut s = Spring::new(5.0, GROW_W);
    s.set_velocity_for_test(-50.0);
    s.clamp_min(10.0);
    assert_bits(s.value(), 10.0);
    assert_bits(s.velocity(), 0.0);
}

#[test]
fn opacity_eps_settles_tighter() {
    let mut s = Spring::new(0.0, OPACITY).with_eps(0.002, 0.01);
    s.retarget(1.0, OPACITY);
    let (_, settle) = run(&mut s, 5.0);
    assert!(settle.is_finite());
    assert!((s.value() - 1.0).abs() < 0.002);
}

/// Exact float equality without tripping `float_cmp`.
fn assert_bits(left: f64, right: f64) {
    assert_eq!(left.to_bits(), right.to_bits(), "{left} != {right}");
}
