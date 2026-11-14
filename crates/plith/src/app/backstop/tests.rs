use super::*;
use std::sync::atomic::{AtomicBool, Ordering};

#[test]
fn work_that_returns_in_time_releases_the_backstop() {
    let fired = AtomicBool::new(false);
    let started = std::time::Instant::now();
    let value = with_backstop(
        Duration::from_secs(30),
        || fired.store(true, Ordering::SeqCst),
        || 7,
    );
    assert_eq!(value, 7);
    assert!(
        !fired.load(Ordering::SeqCst),
        "the exit closure is not called"
    );
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "the timer thread is released at once, not after its timeout"
    );
}

#[test]
fn work_that_overruns_calls_the_exit_closure() {
    let fired = AtomicBool::new(false);
    let value = with_backstop(
        Duration::from_millis(20),
        || fired.store(true, Ordering::SeqCst),
        || {
            thread::sleep(Duration::from_millis(300));
            "done"
        },
    );
    assert_eq!(value, "done");
    assert!(fired.load(Ordering::SeqCst), "the exit closure was called");
}
