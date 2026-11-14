//! Shared pieces of the write verification: the verdict, the restore guard, event waiting.

use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

const EVENT_WAIT: Duration = Duration::from_secs(2);

pub enum Verdict {
    Pass,
    Fail(String),
    Skipped(String),
    Planned,
}

/// Runs `undo` once: from `restore_now`, or on drop (early return or panic unwinding).
pub struct Guard<F: FnMut()> {
    undo: F,
    armed: bool,
}

impl<F: FnMut()> Guard<F> {
    pub fn new(undo: F) -> Self {
        Self { undo, armed: true }
    }

    pub fn restore_now(&mut self) {
        if self.armed {
            self.armed = false;
            (self.undo)();
        }
    }
}

impl<F: FnMut()> Drop for Guard<F> {
    fn drop(&mut self) {
        self.restore_now();
    }
}

/// Waits for a change notification that satisfies `wanted`; reports whether one arrived.
pub fn event_arrived<T>(events: &Receiver<T>, wanted: impl Fn(&T) -> bool) -> bool {
    let deadline = Instant::now() + EVENT_WAIT;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match events.recv_timeout(left) {
            Ok(value) if wanted(&value) => return true,
            Ok(_) => {}
            Err(_) => return false,
        }
    }
}

pub fn drain<T>(events: &Receiver<T>) {
    while events.try_recv().is_ok() {}
}

pub fn yes_no(arrived: bool) -> &'static str {
    if arrived { "yes" } else { "NO" }
}

pub fn close(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

pub fn planned(text: &str) -> Verdict {
    println!("WOULD: {text}");
    Verdict::Planned
}

pub fn judge(changed: bool, restored: bool) -> Verdict {
    match (changed, restored) {
        (true, true) => Verdict::Pass,
        (false, true) => Verdict::Fail("the change did not take effect (restored)".into()),
        (_, false) => Verdict::Fail("the original value was NOT restored".into()),
    }
}
