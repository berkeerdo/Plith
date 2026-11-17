use super::*;

fn secs(base: Instant, n: u64) -> Instant {
    base + Duration::from_secs(n)
}

#[test]
fn first_failure_retries_after_spacing() {
    let base = Instant::now();
    let mut relaunch = Relaunch::new();
    assert_eq!(relaunch.on_failure(base), Next::At(base + LAUNCH_SPACING));
}

#[test]
fn fifth_failure_in_a_minute_gives_up() {
    let base = Instant::now();
    let mut relaunch = Relaunch::new();
    for n in 0..4 {
        let now = secs(base, n * 3);
        assert_eq!(relaunch.on_failure(now), Next::At(now + LAUNCH_SPACING));
    }
    assert_eq!(relaunch.on_failure(secs(base, 12)), Next::GiveUp);
}

#[test]
fn failures_older_than_the_window_do_not_count() {
    let base = Instant::now();
    let mut relaunch = Relaunch::new();
    for n in 0..4 {
        relaunch.on_failure(secs(base, n));
    }
    let later = secs(base, 200);
    assert_eq!(relaunch.on_failure(later), Next::At(later + LAUNCH_SPACING));
}

#[test]
fn connect_clears_history() {
    let base = Instant::now();
    let mut relaunch = Relaunch::new();
    for n in 0..4 {
        relaunch.on_failure(secs(base, n));
    }
    relaunch.on_connected();
    let now = secs(base, 5);
    assert_eq!(relaunch.on_failure(now), Next::At(now + LAUNCH_SPACING));
}

#[test]
fn hide_only_while_link_up() {
    assert_eq!(
        wants_suppression(FlyoutMode::Hide, true),
        Some(SuppressMode::Hide)
    );
    assert_eq!(wants_suppression(FlyoutMode::Hide, false), None);
}

#[test]
fn dry_run_ignores_link() {
    assert_eq!(
        wants_suppression(FlyoutMode::DryRun, true),
        Some(SuppressMode::DryRun)
    );
    assert_eq!(
        wants_suppression(FlyoutMode::DryRun, false),
        Some(SuppressMode::DryRun)
    );
}

#[test]
fn off_is_none() {
    assert_eq!(wants_suppression(FlyoutMode::Off, true), None);
    assert_eq!(wants_suppression(FlyoutMode::Off, false), None);
}
