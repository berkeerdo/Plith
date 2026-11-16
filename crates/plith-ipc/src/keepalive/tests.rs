use super::{IDLE_TIMEOUT, KEEPALIVE, ping_due};
use std::time::{Duration, Instant};

#[test]
fn keepalive_is_well_inside_idle() {
    // Two pings fit in one idle window, so one late ping never drops a live client.
    assert!(KEEPALIVE * 2 < IDLE_TIMEOUT);
}

#[test]
fn ping_is_due_only_after_the_keepalive_interval() {
    let start = Instant::now();
    assert!(!ping_due(start, start));
    let just_before = start + KEEPALIVE.saturating_sub(Duration::from_millis(1));
    assert!(!ping_due(start, just_before));
    assert!(ping_due(start, start + KEEPALIVE));
    assert!(
        !ping_due(start + KEEPALIVE, start),
        "a clock step back is not due"
    );
}
