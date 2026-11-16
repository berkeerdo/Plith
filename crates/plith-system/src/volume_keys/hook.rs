//! `VolumeKeyHook`: the `WH_KEYBOARD_LL` hook and the thread that pumps for it.

use std::cell::{OnceCell, RefCell};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::time::Instant;

use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, HHOOK, KBDLLHOOKSTRUCT, SetWindowsHookExW, UnhookWindowsHookEx, WH_KEYBOARD_LL,
};

use super::{HeldKeys, VolumeKey, handle_key_event};
use crate::flyout::SuppressionWindow;
use crate::hook_thread::HookThread;

/// Signals queued before a slow consumer starts dropping new ones.
const QUEUE: usize = 8;

thread_local! {
    /// The sender the proc uses; set once on the hook thread, read without a lock.
    static SENDER: OnceCell<SyncSender<VolumeKey>> = const { OnceCell::new() };
    /// Which keys are held, to collapse auto-repeat; only the hook thread touches it.
    static HELD: RefCell<HeldKeys> = const { RefCell::new(HeldKeys { mute: None, down: None, up: None }) };
    /// The suppression window the proc opens inline, if the owner gave one.
    static WINDOW: OnceCell<Arc<SuppressionWindow>> = const { OnceCell::new() };
}

/// The installed hook; unhooks on drop, on the thread that installed it.
struct Installed(HHOOK);

impl Drop for Installed {
    fn drop(&mut self) {
        // SAFETY: the handle came from `SetWindowsHookExW` and is released exactly once.
        let _ = unsafe { UnhookWindowsHookEx(self.0) };
    }
}

/// Hook proc. Reads the key code, hands it to [`handle_key_event`] and always passes the
/// event on. Nothing else happens here.
unsafe extern "system" fn proc(n_code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
    if n_code >= 0 && l_param.0 != 0 {
        // SAFETY: for `n_code >= 0` Windows passes a valid `KBDLLHOOKSTRUCT` pointer.
        let vk = unsafe { (*(l_param.0 as *const KBDLLHOOKSTRUCT)).vkCode };
        SENDER.with(|cell| {
            if let Some(tx) = cell.get() {
                WINDOW.with(|window| {
                    let window = window.get().map(Arc::as_ref);
                    HELD.with(|held| match held.try_borrow_mut() {
                        Ok(mut held) => {
                            handle_key_event(
                                n_code,
                                w_param.0,
                                vk,
                                &mut held,
                                tx,
                                window,
                                Instant::now,
                            );
                        }
                        // Re-entered while borrowed: skip the collapse, never panic.
                        Err(_) => handle_key_event(
                            n_code,
                            w_param.0,
                            vk,
                            &mut HeldKeys::default(),
                            tx,
                            window,
                            Instant::now,
                        ),
                    });
                });
            }
        });
    }
    // SAFETY: forwarding the unchanged arguments; the hook handle argument is ignored.
    unsafe { CallNextHookEx(None, n_code, w_param, l_param) }
}

/// Owner of the volume key hook. Dropping it (or [`stop`](Self::stop)) unhooks and joins.
#[derive(Debug)]
pub struct VolumeKeyHook {
    thread: HookThread,
}

impl VolumeKeyHook {
    /// Installs the hook on a new message thread. The receiver gets one [`VolumeKey`] per press
    /// (auto-repeat collapsed; a lost key up is forgiven after 1.5 s); keys are never swallowed. With a `window` (from
    /// [`FlyoutSuppressor::suppression_window`](crate::FlyoutSuppressor::suppression_window))
    /// the hook proc opens it inline on every volume key, before the signal is queued.
    ///
    /// # Errors
    /// The text says why the thread or the hook could not start.
    pub fn start(
        window: Option<Arc<SuppressionWindow>>,
    ) -> Result<(Self, Receiver<VolumeKey>), String> {
        let (tx, rx) = sync_channel(QUEUE);
        let thread = HookThread::spawn("plith-volume-keys", move || {
            SENDER.with(|cell| {
                let _ = cell.set(tx);
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
        Ok((Self { thread }, rx))
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
