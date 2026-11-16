//! The small pieces `app::run` is made of: the render sinks, the
//! watchdog and the startup and exit lines.

use super::state::App;
use crate::demo::sleep_stoppable;
use crate::log::LogFile;
use plith_core::ambient::LiveValues;
use plith_core::launch::Launch;
use plith_core::layout::SurfaceLayout;
use plith_render::{Fatal, RenderSinks};
use plith_shell::{MonitorInfo, QuitHandle, ShapePoster, Shell, is_remote_session};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};

/// How long a demo or an idle check may run before the watchdog closes the window.
pub const WATCHDOG_SECS: u64 = 75;

/// Seconds the watchdog waits after asking the window to close before it ends the process.
pub const WATCHDOG_GRACE_SECS: u64 = 5;

/// Exit code when the render thread died.
pub const EXIT_RENDER_FATAL: i32 = 1;
/// Exit code when the user is busy (fullscreen game, presentation) and a run was skipped.
pub const EXIT_USER_BUSY: i32 = 3;
/// Exit code when the watchdog had to close the window.
pub const EXIT_WATCHDOG: i32 = 4;

/// The mode word of the startup line.
#[must_use]
pub fn mode_text(launch: &Launch) -> String {
    match (&launch.demo, launch.idle_check_secs) {
        (Some(script), _) => format!("demo {} steps x{}", script.steps.len(), script.repeat),
        (None, Some(secs)) => format!("idle-check {secs}"),
        (None, None) => "normal".to_string(),
    }
}

/// The first line of every run.
#[must_use]
pub fn start_line(launch: &Launch, monitor: &MonitorInfo, layout: &SurfaceLayout) -> String {
    let s = layout.surface;
    format!(
        "start: {}x{} px at ({},{}), scale {}, {} Hz, clock request {}, remote {}, mode {}, levers {}, tune {}",
        s.width,
        s.height,
        s.x,
        s.y,
        monitor.scale,
        monitor.hz,
        format!("{:?}", launch.clock).to_lowercase(),
        is_remote_session(),
        mode_text(launch),
        launch.levers.describe(),
        launch.tune.name,
    )
}

/// Which exit code wins: a dead renderer, then the watchdog, then the idle verdict, then the
/// message loop's own code. When the render join hangs after a watchdog quit, the 3 s close
/// backstop fires before the watchdog's 5 s grace, so the exit code is 1 (render fatal), not 4.
#[must_use]
pub fn exit_code(loop_code: i32, fatal: bool, watchdog: bool, idle: Option<bool>) -> i32 {
    if fatal {
        EXIT_RENDER_FATAL
    } else if watchdog {
        EXIT_WATCHDOG
    } else if let Some(ok) = idle {
        i32::from(!ok)
    } else {
        loop_code
    }
}

/// The log line for a foreground change that reached the topmost fight.
#[must_use]
pub fn raised_line(foreground: &str, above: Option<&str>, immediate: bool) -> String {
    if immediate {
        format!(
            "topmost raise (foreground: {foreground}; above: {})",
            above.unwrap_or("none")
        )
    } else {
        format!("topmost retry armed (foreground: {foreground})")
    }
}

/// Logs why the render thread ended and sets `fatal`. Returns true when the window should be
/// asked to close. A plain error while the window is already closing is part of a clean close
/// and changes nothing else; a panic always counts, closing or not.
pub fn handle_fatal(reason: &Fatal, closing: bool, log: &LogFile, fatal: &AtomicBool) -> bool {
    if closing && matches!(reason, Fatal::Error(_)) {
        log.line(&format!(
            "render stopped while closing: {}",
            reason.message()
        ));
        return false;
    }
    log.line(&format!("render fatal: {}", reason.message()));
    fatal.store(true, Ordering::Release);
    true
}

/// The render thread's callbacks: shapes to the UI thread, motion and ambient reports and a
/// fatal end to the log, live values from `sample`. A fatal end closes the window and sets
/// `fatal`, with one exception: a `Fatal::Error`
/// while the window is already closing is a clean close (a present that fails on a window being
/// destroyed) and only logs `render stopped while closing:`. A `Fatal::Panic` always logs
/// `render fatal:` and sets `fatal`, so the exit code is 1.
pub fn sinks(
    poster: ShapePoster,
    log: &Arc<LogFile>,
    quit: QuitHandle,
    fatal: &Arc<AtomicBool>,
    closing: &Arc<AtomicBool>,
    sample: Box<dyn FnMut() -> LiveValues + Send>,
) -> RenderSinks {
    let (motion_log, fatal_log, note_log) = (Arc::clone(log), Arc::clone(log), Arc::clone(log));
    let ambient_log = Arc::clone(log);
    let (fatal, closing) = (Arc::clone(fatal), Arc::clone(closing));
    RenderSinks {
        on_shape: Box::new(move |frame| poster.post(frame)),
        on_motion: Box::new(move |report| {
            motion_log.line(&report.to_string());
            for line in report.detail_lines() {
                motion_log.line(&line);
            }
        }),
        on_note: Box::new(move |line| note_log.line(&line)),
        sample,
        on_ambient: Box::new(move |report| ambient_log.line(&report.to_string())),
        on_fatal: Box::new(move |reason| {
            if handle_fatal(&reason, closing.load(Ordering::Acquire), &fatal_log, &fatal) {
                quit.quit();
            }
        }),
    }
}

/// What happens when a window callback panics: the payload is logged as `ui fatal`, the same
/// `fatal` flag the render sink uses is set (so the exit code is 1) and the render thread is
/// stopped and joined, because the shell is about to close the window. Runs on the UI thread,
/// after the failed callback's borrow of the app was released by the unwind.
pub fn on_panic(
    log: &Arc<LogFile>,
    fatal: &Arc<AtomicBool>,
    app: &Rc<RefCell<App>>,
) -> Box<dyn Fn(String)> {
    let (log, fatal, app) = (Arc::clone(log), Arc::clone(fatal), Rc::clone(app));
    Box::new(move |message| {
        log.line(&format!("ui fatal: {message}"));
        fatal.store(true, Ordering::Release);
        match app.try_borrow() {
            Ok(app) => app.close_render(),
            Err(_) => log.line("ui fatal: app busy, render not stopped here"),
        }
    })
}

/// Closes the window after [`WATCHDOG_SECS`] unless `stop` ends the wait first.
pub fn spawn_watchdog(
    stop: &Arc<AtomicBool>,
    quit: QuitHandle,
    log: &Arc<LogFile>,
    fired: &Arc<AtomicBool>,
) -> JoinHandle<()> {
    let (stop, log, fired) = (Arc::clone(stop), Arc::clone(log), Arc::clone(fired));
    thread::spawn(move || {
        if sleep_stoppable(WATCHDOG_SECS * 1000, &stop) {
            log.line(&format!("watchdog: quit after {WATCHDOG_SECS} s"));
            fired.store(true, Ordering::Release);
            quit.quit();
            if !sleep_stoppable(WATCHDOG_GRACE_SECS * 1000, &stop) {
                return;
            }
            log.line("watchdog: forced exit");
            // `exit` skips destructors, so the queued lines would be lost without this.
            log.flush();
            std::process::exit(EXIT_WATCHDOG);
        }
    })
}

/// The hit-test cost line written at exit.
#[must_use]
pub fn hit_line(shell: &Shell) -> String {
    let (count, mean, max) = shell.hit_stats();
    format!(
        "hit: {count} updates, mean {mean:.3} ms, max {max:.3} ms, errors {}",
        shell.hit_errors()
    )
}

#[cfg(test)]
mod tests;
