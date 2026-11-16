//! `FlyoutSuppressor`: `WinEvent` hooks (`SHOW`, `CREATE`, `LOCATIONCHANGE`) on their own thread.
//!
//! The callback and its state live in [`state`], the Win32 queries in [`query`]. The hidden set
//! is shared through a [`RestoreHandle`], so a forced exit can restore it from any thread, and
//! mirrored to a journal file, so the next start can restore what a killed run left hidden.

mod query;
mod state;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, sync_channel};
use std::time::Instant;

use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows::Win32::UI::WindowsAndMessaging::{
    EVENT_OBJECT_CREATE, EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_SHOW, WINEVENT_OUTOFCONTEXT,
    WINEVENT_SKIPOWNPROCESS,
};

use self::state::{STATE, State, callback};
use super::{Match, Mode, RestoreHandle, SuppressionWindow, journal};
use crate::guard::FailureSlot;
use crate::hook_thread::HookThread;

/// Matches queued for the owner before new ones are dropped.
const QUEUE: usize = 64;

/// The three installed hooks; unhooked on drop, then the state (and the restore) is dropped.
struct Installed(Vec<HWINEVENTHOOK>);

impl Drop for Installed {
    fn drop(&mut self) {
        for hook in self.0.drain(..) {
            // SAFETY: each handle came from `SetWinEventHook` and is released once.
            let _ = unsafe { UnhookWinEvent(hook) };
        }
        STATE.with(|state| drop(state.borrow_mut().take()));
    }
}

/// Installs the three hooks on the calling thread; on an error the earlier ones are unhooked.
fn install() -> Result<Installed, String> {
    // Constructed first: if a later hook fails, dropping it unhooks the earlier ones.
    let mut installed = Installed(Vec::new());
    for event in [
        EVENT_OBJECT_SHOW,
        EVENT_OBJECT_CREATE,
        EVENT_OBJECT_LOCATIONCHANGE,
    ] {
        // SAFETY: `callback` matches WINEVENTPROC; out of context, so no DLL is needed.
        let hook = unsafe {
            SetWinEventHook(
                event,
                event,
                None,
                Some(callback),
                0,
                0,
                WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
            )
        };
        if hook.0.is_null() {
            return Err(format!("SetWinEventHook failed for event {event:#x}"));
        }
        installed.0.push(hook);
    }
    Ok(installed)
}

/// Hides the shell's volume flyout for a moment after a volume key (see [`Mode`]). Dropping it
/// (or [`stop`](Self::stop)) unhooks and restores every window it hid with `SW_SHOWNA`.
#[derive(Debug)]
pub struct FlyoutSuppressor {
    window: Arc<SuppressionWindow>,
    matches: Receiver<Match>,
    restore: RestoreHandle,
    failure: FailureSlot,
    thread: HookThread,
}

impl FlyoutSuppressor {
    /// Installs the three `WinEvent` hooks on a new message thread. [`Mode::Hide`] journals to
    /// [`default_journal`](Self::default_journal) (see
    /// [`start_with_journal`](Self::start_with_journal)); [`Mode::DryRun`] uses no journal, so a
    /// probe never touches the file of a running app.
    ///
    /// # Errors
    /// The text says why the thread or a hook could not start.
    pub fn start(mode: Mode) -> Result<Self, String> {
        let journal = match mode {
            Mode::Hide => Self::default_journal(),
            Mode::DryRun => None,
        };
        Self::start_with_journal(mode, journal)
    }

    /// [`start`](Self::start) with an explicit journal file. With a journal, the hook thread
    /// first shows every journaled window left hidden by a run that was killed, and only one
    /// that is still an orphan: it is still a window (`IsWindow`), it is not visible, and it
    /// still has the same facts the hide path required (a flyout host class, owned by the
    /// shell process, in a flyout band or an unknown one, see [`crate::flyout::is_orphan`]). A
    /// journal stamped by another boot is discarded
    /// unread. Either way the file is then cleared; after that each hide is written to it and
    /// each restore clears it. A missing or unreadable journal is not an error.
    ///
    /// # Errors
    /// The text says why the thread or a hook could not start.
    pub fn start_with_journal(mode: Mode, journal: Option<PathBuf>) -> Result<Self, String> {
        let window = Arc::new(SuppressionWindow::new(Instant::now()));
        let (tx, matches) = sync_channel(QUEUE);
        let boot = query::current_boot();
        let restore = RestoreHandle::new(journal.clone(), boot, query::post_show);
        let failure = FailureSlot::default();
        let (shared, restore_state, failure_state) =
            (Arc::clone(&window), restore.clone(), failure.clone());
        let thread = HookThread::spawn("plith-flyout", move || {
            let band_fn = query::resolve_band_fn();
            if let Some(path) = &journal {
                let is_orphan = |handle| query::is_orphan(handle, band_fn);
                journal::restore_orphans(path, boot, is_orphan, query::post_show);
            }
            STATE.with(|cell| {
                *cell.borrow_mut() = Some(State {
                    mode,
                    window: shared,
                    restore: restore_state,
                    failure: failure_state,
                    names: HashMap::new(),
                    band_fn,
                    matches: tx,
                });
            });
            install()
        })?;
        Ok(Self {
            window,
            matches,
            restore,
            failure,
            thread,
        })
    }

    /// `%LOCALAPPDATA%\Plith\flyout-hidden.txt`; `None` when the folder cannot be resolved.
    #[must_use]
    pub fn default_journal() -> Option<PathBuf> {
        query::default_journal_path()
    }

    /// The shared suppression window, for [`VolumeKeyHook::start`](crate::VolumeKeyHook::start):
    /// the key hook opens it inline, so no consumer thread sits between a key and the window.
    #[must_use]
    pub fn suppression_window(&self) -> Arc<SuppressionWindow> {
        Arc::clone(&self.window)
    }

    /// A handle that restores every hidden window from any thread, for paths that skip `Drop`
    /// (a backstop or watchdog that calls `std::process::exit`).
    #[must_use]
    pub fn restore_handle(&self) -> RestoreHandle {
        self.restore.clone()
    }

    /// Why the hook stopped working, if it did: the text of a panic in the callback (which
    /// then disabled itself and restored every hidden window), or the message loop error that
    /// ended the hook thread early. `None` while healthy.
    #[must_use]
    pub fn failure(&self) -> Option<String> {
        self.failure.get().or_else(|| self.thread.ended_early())
    }

    /// Opens the suppression window from now (call when a volume key was seen).
    pub fn open_window(&self) {
        self.window.open(Instant::now());
    }

    /// Windows that passed every filter (hidden, or only reported in [`Mode::DryRun`]).
    #[must_use]
    pub fn matches(&self) -> &Receiver<Match> {
        &self.matches
    }

    /// Unhooks, restores every hidden window and joins the thread. Safe to call twice.
    ///
    /// # Errors
    /// The thread could not be told to quit and is still running; it was not joined.
    pub fn stop(&mut self) -> Result<(), String> {
        self.thread.stop()
    }
}

impl Drop for FlyoutSuppressor {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
