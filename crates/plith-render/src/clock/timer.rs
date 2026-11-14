//! The fallback clock: a high-resolution waitable timer at the nominal period, on its own
//! schedule (no vblank; the only clock in a remote session).

use super::{ClockError, FrameSource, WAIT_TIMEOUT_MS, seconds_since};
use plith_core::clock_choice::ClockKind;
use std::time::Instant;
use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT};
use windows::Win32::System::Threading::{
    CREATE_WAITABLE_TIMER_HIGH_RESOLUTION, CreateWaitableTimerExW, SetWaitableTimer,
    TIMER_ALL_ACCESS, WaitForSingleObject,
};
use windows::core::PCWSTR;

/// Ticks every nominal period from its own `next_due` schedule.
#[derive(Debug)]
pub struct TimerClock {
    handle: HANDLE,
    period: f64,
    next_due: Option<f64>,
    origin: Instant,
}

/// A relative due time in 100 ns units (negative means relative), at least one unit.
// Allowed: `secs` is under a few periods, far inside i64 range.
#[allow(clippy::cast_possible_truncation)]
fn relative_due(secs: f64) -> i64 {
    -((secs * 1e7).ceil() as i64).max(1)
}

/// The next due time on the `period` grid through `last`, strictly after `now`.
fn next_after(last: Option<f64>, now: f64, period: f64) -> f64 {
    let mut due = last.map_or(now + period, |d| d + period);
    if due <= now {
        due += ((now - due) / period).floor().mul_add(period, period);
    }
    due
}

impl TimerClock {
    /// Creates the timer for a display of `hz`.
    ///
    /// # Errors
    /// Returns the failing `CreateWaitableTimerExW` error.
    pub fn new(hz: f64, origin: Instant) -> Result<Self, ClockError> {
        // SAFETY: no security attributes and no name; the flag and access mask are valid.
        let handle = unsafe {
            CreateWaitableTimerExW(
                None,
                PCWSTR::null(),
                CREATE_WAITABLE_TIMER_HIGH_RESOLUTION,
                TIMER_ALL_ACCESS.0,
            )?
        };
        Ok(Self {
            handle,
            period: 1.0 / hz,
            next_due: None,
            origin,
        })
    }
}

impl FrameSource for TimerClock {
    fn wait(&mut self) -> Result<f64, ClockError> {
        let now = seconds_since(self.origin);
        let due = next_after(self.next_due, now, self.period);
        self.next_due = Some(due);
        let relative = relative_due(due - now);
        // SAFETY: `handle` is the live timer this clock owns; `relative` outlives the call;
        // no completion routine is passed. The wait is finite.
        let status = unsafe {
            SetWaitableTimer(self.handle, &raw const relative, 0, None, None, false)?;
            WaitForSingleObject(self.handle, WAIT_TIMEOUT_MS)
        };
        match status {
            WAIT_OBJECT_0 => Ok(seconds_since(self.origin)),
            WAIT_TIMEOUT => Err(ClockError::Timeout),
            other => Err(ClockError::Status(other.0)),
        }
    }

    fn kind(&self) -> ClockKind {
        ClockKind::Timer
    }
}

impl Drop for TimerClock {
    fn drop(&mut self) {
        // SAFETY: `handle` was created by this clock and is closed exactly once, here.
        let _ = unsafe { CloseHandle(self.handle) };
    }
}
