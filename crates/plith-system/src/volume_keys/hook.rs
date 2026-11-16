//! `VolumeKeyHook`: the `WH_KEYBOARD_LL` hook and the thread that pumps for it.
//!
//! The proc's body runs inside [`contain_text`]: a panic sets a thread-local disabled flag, so
//! every later event goes straight to `CallNextHookEx`, and records the text for the owner.

use std::cell::{Cell, OnceCell, RefCell};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::time::Instant;

use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, HHOOK, KBDLLHOOKSTRUCT, SetWindowsHookExW, UnhookWindowsHookEx, WH_KEYBOARD_LL,
};

use super::{HeldKeys, VolumeKey, handle_key_event};
use crate::flyout::SuppressionWindow;
use crate::guard::{FailureSlot, contain_text};
use crate::hook_thread::HookThread;

/// Signals queued before a slow consumer starts dropping new ones.
const QUEUE: usize = 8;

thread_local! {
    /// The sender the proc uses; set once on the hook thread, read without a lock.
    static SENDER: OnceCell<SyncSender<VolumeKey>> = const { OnceCell::new() };
    /// Which keys are held, to collapse auto-repeat; only the hook thread touches it.
    static HELD: RefCell<HeldKeys> = const { RefCell::new(HeldKeys::new()) };
    /// The suppression window the proc opens inline, if the owner gave one.
    static WINDOW: OnceCell<Arc<SuppressionWindow>> = const { OnceCell::new() };
    /// Set after a contained panic; the proc then only passes events on.
    static DISABLED: Cell<bool> = const { Cell::new(false) };
    /// Where a contained panic is reported to the owner.
    static FAILURE: OnceCell<FailureSlot> = const { OnceCell::new() };
}

/// The installed hook; unhooks on drop, on the thread that installed it.
struct Installed(HHOOK);

impl Drop for Installed {
    fn drop(&mut self) {
        // SAFETY: the handle came from `SetWindowsHookExW` and is released exactly once.
        let _ = unsafe { UnhookWindowsHookEx(self.0) };
    }
}

/// Hands one key event to [`handle_key_event`] with the thread's state.
fn classify(n_code: i32, w_param: usize, vk: u32) {
    let _ = SENDER.try_with(|cell| {
        let Some(tx) = cell.get() else {
            return;
        };
        let _ = WINDOW.try_with(|window| {
            let window = window.get().map(Arc::as_ref);
            HELD.with(|held| match held.try_borrow_mut() {
                Ok(mut held) => {
                    handle_key_event(n_code, w_param, vk, &mut held, tx, window, Instant::now);
                }
                // Re-entered while borrowed: skip the collapse, never panic.
                Err(_) => handle_key_event(
                    n_code,
                    w_param,
                    vk,
                    &mut HeldKeys::new(),
                    tx,
                    window,
                    Instant::now,
                ),
            });
        });
    });
}

/// After a contained panic: pass every later event on untouched and tell the owner why.
fn fail(text: String) {
    DISABLED.with(|disabled| disabled.set(true));
    // `try_with`: these thread locals have destructors, so during thread teardown they may be
    // gone; skipping is fine, a panic here would abort.
    let _ = FAILURE.try_with(|slot| {
        if let Some(slot) = slot.get() {
            slot.record(text);
        }
    });
}

/// Hook proc. Reads the key code, hands it to [`handle_key_event`] and always passes the
/// event on. The disabled flag is checked first; nothing else happens here.
unsafe extern "system" fn proc(n_code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
    if n_code >= 0 && l_param.0 != 0 && !DISABLED.with(Cell::get) {
        // SAFETY: for `n_code >= 0` Windows passes a valid `KBDLLHOOKSTRUCT` pointer.
        let vk = unsafe { (*(l_param.0 as *const KBDLLHOOKSTRUCT)).vkCode };
        if let Some(text) = contain_text(|| classify(n_code, w_param.0, vk)) {
            fail(text);
        }
    }
    // SAFETY: forwarding the unchanged arguments; the hook handle argument is ignored.
    unsafe { CallNextHookEx(None, n_code, w_param, l_param) }
}

/// Owner of the volume key hook. Dropping it (or [`stop`](Self::stop)) unhooks and joins.
#[derive(Debug)]
pub struct VolumeKeyHook {
    thread: HookThread,
    failure: FailureSlot,
}

impl VolumeKeyHook {
    /// Installs the hook on a new message thread. The receiver gets one [`VolumeKey`] per press
    /// (auto-repeat collapsed; a lost key up is forgiven after 1.5 s); keys are never
    /// swallowed. With a `window` (from
    /// [`FlyoutSuppressor::suppression_window`](crate::FlyoutSuppressor::suppression_window))
    /// the hook proc opens it inline on every volume key down, before the signal is queued;
    /// a key up opens nothing.
    ///
    /// # Errors
    /// The text says why the thread or the hook could not start.
    pub fn start(
        window: Option<Arc<SuppressionWindow>>,
    ) -> Result<(Self, Receiver<VolumeKey>), String> {
        let (tx, rx) = sync_channel(QUEUE);
        let failure = FailureSlot::default();
        let failure_on_thread = failure.clone();
        let thread = HookThread::spawn("plith-volume-keys", move || {
            SENDER.with(|cell| {
                let _ = cell.set(tx);
            });
            FAILURE.with(|cell| {
                let _ = cell.set(failure_on_thread);
            });
            if let Some(window) = window {
                WINDOW.with(|cell| {
                    let _ = cell.set(window);
                });
            }
            // SAFETY: `proc` has the HOOKPROC signature; a global low level hook needs no
            // module handle and thread id 0.
            unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(proc), None, 0) }
                .map(Installed)
                .map_err(|e| format!("SetWindowsHookExW failed: {e}"))
        })?;
        Ok((Self { thread, failure }, rx))
    }

    /// Why the hook stopped classifying keys, if it did: the text of a panic in the proc (which
    /// then only passes events on), or the message loop error that ended the hook thread early.
    /// `None` while healthy.
    #[must_use]
    pub fn failure(&self) -> Option<String> {
        self.failure.get().or_else(|| self.thread.ended_early())
    }

    /// Unhooks and joins the thread. Safe to call twice.
    ///
    /// # Errors
    /// The thread could not be told to quit and is still running; it was not joined.
    pub fn stop(&mut self) -> Result<(), String> {
        self.thread.stop()
    }
}

impl Drop for VolumeKeyHook {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
