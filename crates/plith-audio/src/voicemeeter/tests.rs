//! Unit tests for the pure Voicemeeter layer.

use super::*;
use std::time::{Duration, Instant};

#[test]
fn kind_counts() {
    assert_eq!(VmKind::from_api(0), None);
    assert_eq!(VmKind::from_api(4), None);
    let expect = [(1, 3, 2), (2, 5, 5), (3, 8, 8)];
    for (code, strips, buses) in expect {
        let kind = VmKind::from_api(code).unwrap();
        assert_eq!(kind.strip_count(), strips);
        assert_eq!(kind.bus_count(), buses);
    }
}

#[test]
fn clamp_gain_range_and_nan() {
    assert!((clamp_gain(-100.0) - MIN_GAIN_DB).abs() < f32::EPSILON);
    assert!((clamp_gain(20.0) - MAX_GAIN_DB).abs() < f32::EPSILON);
    assert!((clamp_gain(-3.5) - -3.5).abs() < f32::EPSILON);
    assert!((clamp_gain(f32::NAN) - MIN_GAIN_DB).abs() < f32::EPSILON);
}

#[test]
fn param_name_format() {
    assert_eq!(param_name(Rail::Strip, 2, "Gain"), "Strip[2].Gain");
    assert_eq!(param_name(Rail::Bus, 0, "Mute"), "Bus[0].Mute");
}

#[test]
fn fallback_labels() {
    assert_eq!(fallback_label(Rail::Strip, 0), "Strip 1");
    assert_eq!(fallback_label(Rail::Strip, 7), "Strip 8");
    assert_eq!(fallback_label(Rail::Bus, 0), "A1");
    assert_eq!(fallback_label(Rail::Bus, 4), "A5");
    assert_eq!(fallback_label(Rail::Bus, 5), "B1");
    assert_eq!(fallback_label(Rail::Bus, 7), "B3");
}

fn secs(n: u64) -> Duration {
    Duration::from_secs(n)
}

#[test]
fn grace_blocks_early_negatives() {
    let t0 = Instant::now();
    let mut s = LoginSession::on_login(0, t0);
    for i in 0..20 {
        let now = t0 + Duration::from_millis(100 * i);
        assert_eq!(s.on_dirty(-1, now), DirtyOutcome::Transient);
    }
}

#[test]
fn five_negatives_after_grace_is_gone() {
    let t0 = Instant::now();
    let mut s = LoginSession::on_login(0, t0);
    let late = t0 + secs(6);
    for _ in 0..4 {
        assert_eq!(s.on_dirty(-1, late), DirtyOutcome::Transient);
    }
    assert_eq!(s.on_dirty(-1, late), DirtyOutcome::EngineGone);
}

#[test]
fn success_resets_streak() {
    let t0 = Instant::now();
    let mut s = LoginSession::on_login(0, t0);
    let late = t0 + secs(6);
    for _ in 0..4 {
        s.on_dirty(-1, late);
    }
    assert_eq!(s.on_dirty(0, late), DirtyOutcome::Clean);
    assert_eq!(s.on_dirty(1, late), DirtyOutcome::Dirty);
    for _ in 0..4 {
        assert_eq!(s.on_dirty(-1, late), DirtyOutcome::Transient);
    }
    assert_eq!(s.on_dirty(-1, late), DirtyOutcome::EngineGone);
}

#[test]
fn rc1_uses_long_grace() {
    let t0 = Instant::now();
    let mut s = LoginSession::on_login(1, t0);
    for _ in 0..10 {
        assert_eq!(s.on_dirty(-1, t0 + secs(20)), DirtyOutcome::Transient);
    }
    let after = t0 + secs(31);
    for _ in 0..4 {
        assert_eq!(s.on_dirty(-1, after), DirtyOutcome::Transient);
    }
    assert_eq!(s.on_dirty(-1, after), DirtyOutcome::EngineGone);
}

#[test]
fn level_to_db_floor() {
    assert!((level_to_db(0.0) - LEVEL_FLOOR_DB).abs() < f32::EPSILON);
    assert!(level_to_db(1.0).abs() < 1e-4);
    assert!((level_to_db(0.5) - -6.0206).abs() < 1e-3);
    assert!((level_to_db(1e-9) - LEVEL_FLOOR_DB).abs() < f32::EPSILON);
    assert!((level_to_db(f32::NAN) - LEVEL_FLOOR_DB).abs() < f32::EPSILON);
}
