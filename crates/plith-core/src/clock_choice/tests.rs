use super::*;

const PERIOD: f64 = 1.0 / 240.0;

#[test]
fn auto_prefers_dcomp_then_dxgi_then_timer() {
    assert_eq!(
        clock_order(ClockRequest::Auto, false),
        vec![ClockKind::Dcomp, ClockKind::Dxgi, ClockKind::Timer]
    );
}

#[test]
fn remote_session_uses_timer_only() {
    for request in [
        ClockRequest::Auto,
        ClockRequest::Dcomp,
        ClockRequest::Dxgi,
        ClockRequest::Timer,
    ] {
        assert_eq!(clock_order(request, true), vec![ClockKind::Timer]);
    }
}

#[test]
fn named_request_goes_first() {
    assert_eq!(
        clock_order(ClockRequest::Dxgi, false),
        vec![ClockKind::Dxgi, ClockKind::Dcomp, ClockKind::Timer]
    );
    assert_eq!(
        clock_order(ClockRequest::Timer, false),
        vec![ClockKind::Timer, ClockKind::Dcomp, ClockKind::Dxgi]
    );
    assert_eq!(
        clock_order(ClockRequest::Dcomp, false),
        vec![ClockKind::Dcomp, ClockKind::Dxgi, ClockKind::Timer]
    );
}

#[test]
fn kind_names_match_the_log_words() {
    assert_eq!(ClockKind::Dcomp.as_str(), "dcomp");
    assert_eq!(ClockKind::Dxgi.as_str(), "dxgi");
    assert_eq!(ClockKind::Timer.as_str(), "timer");
}

#[test]
fn three_consecutive_fast_waits_demote() {
    let mut health = ClockHealth::default();
    assert!(!health.observe(0.0, PERIOD));
    assert!(!health.observe(0.1 * PERIOD, PERIOD));
    assert!(health.observe(0.14 * PERIOD, PERIOD));
}

#[test]
fn a_normal_wait_breaks_the_fast_streak() {
    let mut health = ClockHealth::default();
    assert!(!health.observe(0.0, PERIOD));
    assert!(!health.observe(0.0, PERIOD));
    assert!(!health.observe(PERIOD, PERIOD));
    assert!(!health.observe(0.0, PERIOD));
    assert!(!health.observe(0.0, PERIOD));
    assert!(health.observe(0.0, PERIOD));
}

#[test]
fn a_wait_of_exactly_the_threshold_is_not_fast() {
    let mut health = ClockHealth::default();
    for _ in 0..10 {
        assert!(!health.observe(0.15 * PERIOD, PERIOD));
    }
}
