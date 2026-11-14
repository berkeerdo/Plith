//! The small pieces `app::run` is made of: the shell event handler, the render sinks, the
//! watchdog and the startup and exit lines.

use crate::demo::sleep_stoppable;
use crate::log::LogFile;
use plith_core::launch::Launch;
use plith_core::layout::SurfaceLayout;
use plith_core::scene::{Scene, SceneMailbox};
use plith_render::{RenderSinks, StopHandle};
use plith_shell::{MonitorInfo, QuitHandle, ShapePoster, Shell, ShellEvent, is_remote_session};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
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
        "start: {}x{} px at ({},{}), scale {}, {} Hz, clock request {}, hit {}, remote {}, mode {}, levers {}, tune {}",
        s.width,
        s.height,
        s.x,
        s.y,
        monitor.scale,
        monitor.hz,
        format!("{:?}", launch.clock).to_lowercase(),
        format!("{:?}", launch.hit).to_lowercase(),
        is_remote_session(),
        mode_text(launch),
        launch.levers.describe(),
        launch.tune.name,
    )
}

/// Which exit code wins: a dead renderer, then the watchdog, then the idle verdict, then the
/// message loop's own code.
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

/// The shell event handler. It runs on the UI thread and reads the current scene from the
/// mailbox, so a demo step in between is not overwritten.
///
/// On `Closing` (`WM_CLOSE`, before the window is destroyed) it sets `closing` and asks the
/// render thread to stop, because that thread presents to the window being destroyed.
pub fn event_handler(
    mailbox: &Arc<SceneMailbox>,
    log: &Arc<LogFile>,
    closing: &Arc<AtomicBool>,
    stopper: &Arc<OnceLock<StopHandle>>,
) -> Box<dyn FnMut(ShellEvent)> {
    let (mailbox, log) = (Arc::clone(mailbox), Arc::clone(log));
    let (closing, stopper) = (Arc::clone(closing), Arc::clone(stopper));
    Box::new(move |event| match event {
        ShellEvent::Closing => {
            closing.store(true, Ordering::Release);
            if let Some(stop) = stopper.get() {
                stop.request();
            }
        }
        ShellEvent::Click => {
            mailbox.update(|scene| Scene {
                expanded: !scene.expanded,
                ..scene
            });
        }
        ShellEvent::Suppression(suppression) => {
            mailbox.update(|scene| Scene {
                suppression,
                ..scene
            });
            log.line(&format!(
                "suppression: display_off={} locked={} fullscreen={}",
                suppression.display_off, suppression.locked, suppression.fullscreen
            ));
        }
        ShellEvent::DisplayChanged => log.line("display changed: restart needed"),
        ShellEvent::Raised { culprit } => match culprit {
            Some(class) => log.line(&format!("topmost raise (above: {class})")),
            None => log.line("topmost raise"),
        },
    })
}

/// The render thread's callbacks: shapes to the UI thread, motion reports and a fatal error to
/// the log (a fatal error also closes the window and sets `fatal`, unless the window is already
/// closing: a present that fails on a window being destroyed is part of a clean close).
pub fn sinks(
    poster: ShapePoster,
    log: &Arc<LogFile>,
    quit: QuitHandle,
    fatal: &Arc<AtomicBool>,
    closing: &Arc<AtomicBool>,
) -> RenderSinks {
    let (motion_log, fatal_log, note_log) = (Arc::clone(log), Arc::clone(log), Arc::clone(log));
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
        on_fatal: Box::new(move |message| {
            if closing.load(Ordering::Acquire) {
                fatal_log.line(&format!("render stopped while closing: {message}"));
                return;
            }
            fatal_log.line(&format!("render fatal: {message}"));
            fatal.store(true, Ordering::Release);
            quit.quit();
        }),
    }
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
