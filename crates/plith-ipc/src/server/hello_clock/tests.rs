use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use super::HelloClock;

/// Runs `body` with a clock whose thread counts its cancels.
fn with_clock(body: impl FnOnce(&HelloClock, &AtomicU32)) {
    let clock = HelloClock::default();
    let cancels = AtomicU32::new(0);
    std::thread::scope(|scope| {
        scope.spawn(|| {
            clock.run(|| {
                cancels.fetch_add(1, Ordering::SeqCst);
            });
        });
        body(&clock, &cancels);
        clock.quit();
    });
}

#[test]
fn a_passed_deadline_expires_and_cancels_until_disarmed() {
    with_clock(|clock, cancels| {
        clock.arm(Instant::now() + Duration::from_millis(20));
        std::thread::sleep(Duration::from_millis(120));
        assert!(clock.expired());
        assert!(cancels.load(Ordering::SeqCst) >= 2, "cancels repeat");
        assert!(clock.disarm(), "disarm reports the expiry");
        let after = cancels.load(Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(
            cancels.load(Ordering::SeqCst),
            after,
            "no cancel after disarm"
        );
        assert!(!clock.expired());
    });
}

#[test]
fn a_disarmed_clock_never_cancels() {
    with_clock(|clock, cancels| {
        clock.arm(Instant::now() + Duration::from_millis(40));
        assert!(!clock.disarm());
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(cancels.load(Ordering::SeqCst), 0);
    });
}

#[test]
fn rearming_starts_a_fresh_deadline() {
    with_clock(|clock, cancels| {
        clock.arm(Instant::now() + Duration::from_millis(10));
        std::thread::sleep(Duration::from_millis(60));
        assert!(clock.expired());
        clock.arm(Instant::now() + Duration::from_secs(5));
        assert!(!clock.expired(), "a new arm clears the old expiry");
        let before = cancels.load(Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(cancels.load(Ordering::SeqCst), before);
    });
}
