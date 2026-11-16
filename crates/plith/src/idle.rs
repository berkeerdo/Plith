//! The idle check: at rest the render thread must stay parked, with no wake-ups and no CPU.

use crate::demo::sleep_stoppable;
use crate::log::LogFile;
use crate::services::start::{ServiceId, StartBoard};
use plith_render::RenderHandle;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::FILETIME;
use windows::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};

/// Time for the startup frame to settle before the first reading.
pub const SETTLE_MS: u64 = 1500;
/// The longest wait for the service starts (brightness alone may take 15 s).
pub const START_CAP: Duration = Duration::from_secs(20);
/// How often the start wait looks at the board.
const START_POLL_MS: u64 = 50;

/// How the wait for the service starts ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartWait {
    /// True when every start returned; false when the cap ended the wait.
    pub all: bool,
    /// How long the wait took.
    pub waited: Duration,
    /// The services whose start returned, in start order.
    pub up: Vec<ServiceId>,
}

/// Waits until every start on `board` returned or `cap` passed. `None` when `stop` ends it.
#[must_use]
pub fn wait_for_starts(board: &StartBoard, cap: Duration, stop: &AtomicBool) -> Option<StartWait> {
    let begun = Instant::now();
    loop {
        let all = board.all_up();
        let waited = begun.elapsed();
        if all || waited >= cap {
            return Some(StartWait {
                all,
                waited,
                up: board.up(),
            });
        }
        if !sleep_stoppable(START_POLL_MS, stop) {
            return None;
        }
    }
}

/// The log line of a start wait: how it ended, after how long, and which services were up.
#[must_use]
pub fn start_line(wait: &StartWait) -> String {
    let up: Vec<&str> = wait.up.iter().map(|id| id.name()).collect();
    let how = if wait.all {
        "every service started"
    } else {
        "start cap reached"
    };
    format!(
        "idle: {how} after {} ms; up: {}",
        wait.waited.as_millis(),
        if up.is_empty() {
            "none".to_string()
        } else {
            up.join(", ")
        }
    )
}

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
/// thread never woke. With real services (`board`), the first reading waits until every start
/// returned (or [`START_CAP`]) and logs which were up, since a start may change the pill.
/// Returns false without a line when `stop` ends the wait early.
#[must_use]
pub fn run(
    secs: u64,
    render: &RenderHandle,
    log: &LogFile,
    stop: &AtomicBool,
    board: Option<&StartBoard>,
) -> bool {
    if let Some(board) = board {
        let Some(wait) = wait_for_starts(board, START_CAP, stop) else {
            return false;
        };
        log.line(&start_line(&wait));
    }
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
    use super::{StartWait, report_line, start_line, wait_for_starts};
    use crate::services::start::{ServiceId, StartBoard};
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use std::time::Duration;

    #[test]
    fn report_line_matches_the_pinned_format() {
        assert_eq!(
            report_line(60, 0, 0.0),
            "idle 60 s: 0 render wake-ups, 0.0 ms process CPU"
        );
    }

    #[test]
    fn a_full_board_needs_no_wait() {
        let board = StartBoard::default();
        for id in ServiceId::ALL {
            board.mark(id);
        }
        let wait = wait_for_starts(&board, Duration::from_secs(5), &AtomicBool::new(false));
        let wait = wait.unwrap();
        assert!(wait.all);
        assert!(wait.waited < Duration::from_secs(1));
        assert_eq!(wait.up, ServiceId::ALL);
    }

    #[test]
    fn the_wait_ends_when_the_last_start_reports() {
        let board = Arc::new(StartBoard::default());
        let late = Arc::clone(&board);
        let marker = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(30));
            for id in ServiceId::ALL {
                late.mark(id);
            }
        });
        let wait = wait_for_starts(&board, Duration::from_secs(5), &AtomicBool::new(false));
        marker.join().unwrap();
        assert!(wait.unwrap().all);
    }

    #[test]
    fn the_wait_is_capped_and_says_who_was_up() {
        let board = StartBoard::default();
        board.mark(ServiceId::Endpoints);
        let wait = wait_for_starts(&board, Duration::from_millis(60), &AtomicBool::new(false));
        let wait = wait.unwrap();
        assert!(!wait.all);
        assert!(wait.waited >= Duration::from_millis(60));
        assert_eq!(
            start_line(&wait),
            format!(
                "idle: start cap reached after {} ms; up: endpoints",
                wait.waited.as_millis()
            )
        );
    }

    #[test]
    fn a_stop_ends_the_wait_without_a_result() {
        let board = StartBoard::default();
        let stop = AtomicBool::new(true);
        assert!(wait_for_starts(&board, Duration::from_secs(5), &stop).is_none());
    }

    #[test]
    fn the_line_lists_every_service_when_all_started() {
        let wait = StartWait {
            all: true,
            waited: Duration::from_millis(8200),
            up: ServiceId::ALL.to_vec(),
        };
        assert_eq!(
            start_line(&wait),
            "idle: every service started after 8200 ms; up: voicemeeter, endpoints, \
             microphone, media, brightness"
        );
    }
}
