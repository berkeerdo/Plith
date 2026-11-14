//! The idle check: at rest the render thread must stay parked, with no wake-ups and no CPU.

use crate::demo::sleep_stoppable;
use crate::log::LogFile;
use plith_render::RenderHandle;
use std::sync::atomic::AtomicBool;
use windows::Win32::Foundation::FILETIME;
use windows::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};

/// Time for the startup frame to settle before the first reading.
const SETTLE_MS: u64 = 1500;

fn ticks(t: FILETIME) -> u64 {
    (u64::from(t.dwHighDateTime) << 32) | u64::from(t.dwLowDateTime)
}

/// Kernel plus user CPU time of this process, in milliseconds (0.0 when the call fails).
#[allow(clippy::cast_precision_loss)]
fn process_cpu_ms() -> f64 {
    let mut ignored = FILETIME::default();
    let (mut kernel, mut user) = (FILETIME::default(), FILETIME::default());
    // SAFETY: the pseudo handle of the current process needs no closing and every out pointer
    // is valid for the call.
    let done = unsafe {
        GetProcessTimes(
            GetCurrentProcess(),
            &raw mut ignored,
            &raw mut ignored,
            &raw mut kernel,
            &raw mut user,
        )
    };
    if done.is_err() {
        return 0.0;
    }
    // A FILETIME tick is 100 ns.
    (ticks(kernel) + ticks(user)) as f64 / 10_000.0
}

/// The result line of a check.
#[must_use]
pub fn report_line(secs: u64, wakes: u64, cpu_ms: f64) -> String {
    format!("idle {secs} s: {wakes} render wake-ups, {cpu_ms:.1} ms process CPU")
}

/// Waits `secs` at rest and logs the wake-ups and CPU time spent meanwhile. True when the render
/// thread never woke. Returns false without a line when `stop` ends the wait early.
#[must_use]
pub fn run(secs: u64, render: &RenderHandle, log: &LogFile, stop: &AtomicBool) -> bool {
    if !sleep_stoppable(SETTLE_MS, stop) {
        return false;
    }
    let (wakes_before, cpu_before) = (render.wake_count(), process_cpu_ms());
    if !sleep_stoppable(secs.saturating_mul(1000), stop) {
        return false;
    }
    let wakes = render.wake_count().saturating_sub(wakes_before);
    let cpu = (process_cpu_ms() - cpu_before).max(0.0);
    log.line(&report_line(secs, wakes, cpu));
    wakes == 0
}

#[cfg(test)]
mod tests {
    use super::report_line;

    #[test]
    fn report_line_matches_the_pinned_format() {
        assert_eq!(
            report_line(60, 0, 0.0),
            "idle 60 s: 0 render wake-ups, 0.0 ms process CPU"
        );
    }
}
