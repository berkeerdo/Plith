//! Shared pieces of the write verification: the verdict, the restore guard, event waiting.

use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use windows::Win32::System::Console::SetConsoleCtrlHandler;
use windows::core::BOOL;

const EVENT_WAIT: Duration = Duration::from_secs(2);

static INTERRUPTED: AtomicBool = AtomicBool::new(false);

unsafe extern "system" fn on_ctrl(_kind: u32) -> BOOL {
    INTERRUPTED.store(true, Ordering::SeqCst);
    // Handled: the process keeps running so the drop guards can restore every value.
    BOOL(1)
}

/// Turns Ctrl+C (and console close) into a flag the wait loops check, so a run falls through
/// to the restore guards instead of dying with a value changed.
pub fn install_ctrl_handler() {
    // SAFETY: `on_ctrl` is a valid handler routine that only stores to a static atomic.
    if unsafe { SetConsoleCtrlHandler(Some(on_ctrl), true) }.is_err() {
        println!("warning: could not install the Ctrl+C handler");
    }
}

/// True once Ctrl+C was pressed.
pub fn interrupted() -> bool {
    INTERRUPTED.load(Ordering::SeqCst)
}

/// Printed before the first write.
pub const DO_NOT_INTERRUPT: &str =
    "do not press Ctrl+C; if interrupted, restart Explorer and set the default device back by hand";

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
        if interrupted() {
            return false;
        }
        let left = deadline.saturating_duration_since(Instant::now());
        // Short slices, so Ctrl+C is noticed within a fraction of a second.
        match events.recv_timeout(left.min(Duration::from_millis(100))) {
            Ok(value) if wanted(&value) => return true,
            Ok(_) => {}
            Err(RecvTimeoutError::Timeout) if !left.is_zero() => {}
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

/// What a mute toggle did, judged from the toggle calls' own return values (the event driven
/// snapshot can lag, so it is only a fallback when a toggle returned nothing).
pub struct MuteTrack {
    original: bool,
    /// The mute state the last toggle reported; `None` when unknown.
    current: Cell<Option<bool>>,
}

impl MuteTrack {
    pub fn new(original: bool) -> Self {
        Self {
            original,
            current: Cell::new(Some(original)),
        }
    }

    /// Runs one toggle and records what it returned.
    pub fn toggle(&self, call: impl FnOnce() -> Option<bool>) -> Option<bool> {
        let result = call();
        self.current.set(result);
        result
    }

    /// Toggles back when the last toggle reported the opposite of the original. With an
    /// unknown state it falls back to `snapshot_muted`.
    pub fn restore(
        &self,
        call: impl FnOnce() -> Option<bool>,
        snapshot_muted: impl FnOnce() -> Option<bool>,
    ) {
        let flipped = match self.current.get() {
            Some(state) => state != self.original,
            None => snapshot_muted().is_some_and(|m| m != self.original),
        };
        if flipped {
            self.toggle(call);
        }
    }

    /// The state the last toggle reported, if any.
    pub fn reported(&self) -> Option<bool> {
        self.current.get()
    }

    /// True when the original state is back: by the last toggle's return value, or by the
    /// snapshot when that was unknown.
    pub fn settled(&self, snapshot_muted: Option<bool>) -> bool {
        self.current
            .get()
            .or(snapshot_muted)
            .is_some_and(|m| m == self.original)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restore_follows_the_toggle_return_not_the_snapshot() {
        let track = MuteTrack::new(false);
        assert_eq!(track.toggle(|| Some(true)), Some(true));
        // A stale snapshot still says "unmuted"; the return value decides.
        track.restore(|| Some(false), || Some(false));
        assert!(track.settled(Some(true)));
        assert_eq!(track.reported(), Some(false));
    }

    #[test]
    fn no_restore_when_nothing_flipped() {
        let track = MuteTrack::new(true);
        track.restore(|| panic!("must not toggle"), || Some(false));
        assert!(track.settled(None));
    }

    #[test]
    fn unknown_toggle_falls_back_to_the_snapshot() {
        let track = MuteTrack::new(false);
        assert_eq!(track.toggle(|| None), None);
        track.restore(|| Some(false), || Some(true));
        assert!(track.settled(Some(false)));
        let failed = MuteTrack::new(false);
        failed.toggle(|| None);
        assert!(!failed.settled(Some(true)));
    }
}
