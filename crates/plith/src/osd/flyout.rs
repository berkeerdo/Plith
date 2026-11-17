//! Native flyout suppression for the supervisor: the `FlyoutSuppressor` and the volume key hook
//! that opens its window, started and stopped as the wanted mode changes.

use crate::log::LogFile;
use plith_core::osd::policy::SuppressMode;
use plith_system::flyout::{Match, Mode, RestoreHandle};
use plith_system::{FlyoutSuppressor, VolumeKey, VolumeKeyHook};
use std::sync::mpsc::Receiver;
use std::sync::{Mutex, PoisonError};

/// The restore handle of the running `Mode::Hide` suppressor, for exits that skip `Drop`.
/// A `Mutex` rather than a `OnceLock`: suppression can stop and start again when the link
/// drops and returns, and each start has its own hidden set.
static RESTORE: Mutex<Option<RestoreHandle>> = Mutex::new(None);

/// Shows every flyout the running suppressor hid. A no-op when no `Mode::Hide` suppressor
/// runs. Callable from any thread, also right before `std::process::exit`.
pub fn restore_now() {
    let handle = RESTORE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    if let Some(handle) = handle {
        handle.restore_now();
    }
}

/// Records (or clears) the handle [`restore_now`] uses.
fn set_restore(handle: Option<RestoreHandle>) {
    *RESTORE.lock().unwrap_or_else(PoisonError::into_inner) = handle;
}

/// What to do to get from the running mode to the wanted one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// Nothing changes.
    Keep,
    /// Stop what runs.
    Stop,
    /// Start in this mode (nothing runs).
    Start(SuppressMode),
    /// Stop what runs, then start in this mode.
    Restart(SuppressMode),
}

/// Pure. The change from `running` to `wanted`; `pinned_off` (a start failed this session)
/// allows only stopping.
#[must_use]
pub fn change(
    running: Option<SuppressMode>,
    wanted: Option<SuppressMode>,
    pinned_off: bool,
) -> Change {
    let wanted = if pinned_off { None } else { wanted };
    match (running, wanted) {
        (a, b) if a == b => Change::Keep,
        (Some(_), None) => Change::Stop,
        (None, Some(mode)) => Change::Start(mode),
        (Some(_), Some(mode)) => Change::Restart(mode),
        (None, None) => Change::Keep,
    }
}

/// Pure. The log line for one window the suppressor matched.
#[must_use]
pub fn match_line(found: &Match) -> String {
    let what = if found.hidden {
        "hid"
    } else {
        "matched (not hidden)"
    };
    let band = found
        .facts
        .band
        .map_or_else(|| String::from("unknown"), |band| band.to_string());
    format!(
        "osd: flyout {what}: {} of {} (band {band}, hwnd {:#x})",
        found.facts.class, found.facts.process, found.hwnd
    )
}

/// Pure. The words for a mode in log lines.
#[must_use]
pub fn mode_name(mode: SuppressMode) -> &'static str {
    match mode {
        SuppressMode::DryRun => "dry run",
        SuppressMode::Hide => "hide",
    }
}

/// A running suppressor and the key hook that feeds its window.
struct Active {
    mode: SuppressMode,
    flyout: FlyoutSuppressor,
    keys: VolumeKeyHook,
    presses: Receiver<VolumeKey>,
    flyout_failed: bool,
    keys_failed: bool,
}

/// The supervisor's suppression state for the session.
#[derive(Default)]
pub struct Suppression {
    active: Option<Active>,
    /// A start failed: suppression stays off for the rest of the session.
    pinned_off: bool,
}

impl Suppression {
    /// Starts, stops or switches suppression to match `wanted`.
    pub fn apply(&mut self, wanted: Option<SuppressMode>, log: &LogFile) {
        let running = self.active.as_ref().map(|active| active.mode);
        match change(running, wanted, self.pinned_off) {
            Change::Keep => {}
            Change::Stop => self.stop(log),
            Change::Start(mode) => self.start(mode, log),
            Change::Restart(mode) => {
                self.stop(log);
                self.start(mode, log);
            }
        }
    }

    /// Drains the key presses and the matches and logs a hook failure once each.
    pub fn tick(&mut self, log: &LogFile) {
        let Some(active) = &mut self.active else {
            return;
        };
        // Keys only open the window (inline, in the hook); nothing else reads them.
        for _ in active.presses.try_iter() {}
        for found in active.flyout.matches().try_iter() {
            log.line(&match_line(&found));
        }
        if !active.flyout_failed
            && let Some(failure) = active.flyout.failure()
        {
            active.flyout_failed = true;
            log.line(&format!("osd: flyout hook failed: {failure}"));
        }
        if !active.keys_failed
            && let Some(failure) = active.keys.failure()
        {
            active.keys_failed = true;
            log.line(&format!("osd: volume key hook failed: {failure}"));
        }
    }

    /// Stops the suppressor (which shows every flyout it hid) and then the key hook.
    pub fn stop(&mut self, log: &LogFile) {
        let Some(mut active) = self.active.take() else {
            return;
        };
        if let Err(error) = active.flyout.stop() {
            log.line(&format!("osd: flyout hook did not stop: {error}"));
            // The hook thread did not restore; show what it hid before the handle is let go.
            active.flyout.restore_handle().restore_now();
        }
        if let Err(error) = active.keys.stop() {
            log.line(&format!("osd: volume key hook did not stop: {error}"));
        }
        set_restore(None);
        log.line(&format!(
            "osd: flyout suppression off ({})",
            mode_name(active.mode)
        ));
    }

    /// Starts the suppressor and the key hook; a failure pins suppression off.
    fn start(&mut self, mode: SuppressMode, log: &LogFile) {
        let system_mode = match mode {
            SuppressMode::DryRun => Mode::DryRun,
            SuppressMode::Hide => Mode::Hide,
        };
        let flyout = match FlyoutSuppressor::start(system_mode) {
            Ok(flyout) => flyout,
            Err(error) => return self.pin_off(log, &format!("flyout hook: {error}")),
        };
        let (keys, presses) = match VolumeKeyHook::start(Some(flyout.suppression_window())) {
            Ok(started) => started,
            Err(error) => {
                // Dropping the suppressor unhooks it; it has hidden nothing yet.
                drop(flyout);
                return self.pin_off(log, &format!("volume key hook: {error}"));
            }
        };
        if mode == SuppressMode::Hide {
            set_restore(Some(flyout.restore_handle()));
        }
        log.line(&format!("osd: flyout suppression on ({})", mode_name(mode)));
        self.active = Some(Active {
            mode,
            flyout,
            keys,
            presses,
            flyout_failed: false,
            keys_failed: false,
        });
    }

    /// Logs why a start failed and keeps suppression off for the session.
    fn pin_off(&mut self, log: &LogFile, why: &str) {
        self.pinned_off = true;
        log.line(&format!(
            "osd: flyout suppression not started ({why}); off for this session"
        ));
    }
}

#[cfg(test)]
mod tests;
