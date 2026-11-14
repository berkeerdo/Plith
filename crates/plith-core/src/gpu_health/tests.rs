use super::*;

#[test]
fn one_failure_recreates() {
    let mut health = GpuHealth::default();
    assert_eq!(health.record(true), GpuAction::None);
    assert_eq!(health.record(false), GpuAction::Recreate);
}

#[test]
fn three_failures_fall_back_to_warp() {
    let mut health = GpuHealth::default();
    assert_eq!(health.record(false), GpuAction::Recreate);
    assert_eq!(health.record(false), GpuAction::Recreate);
    assert_eq!(health.record(false), GpuAction::FallBackToWarp);
}

#[test]
fn failures_reset_after_600_good_frames() {
    let mut health = GpuHealth::default();
    health.record(false);
    health.record(false);
    for _ in 0..600 {
        assert_eq!(health.record(true), GpuAction::None);
    }
    assert_eq!(health.record(false), GpuAction::Recreate);
    assert_eq!(health.record(false), GpuAction::Recreate);
}
