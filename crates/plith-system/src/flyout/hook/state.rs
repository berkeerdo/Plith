//! The `WinEvent` callback and the state it touches, both on the hook thread.
//!
//! `LOCATIONCHANGE` fires thousands of times per second, so the callback runs the cheapest
//! filters first (the event shape, the disabled flag, is the suppression window open) and does
//! no I/O, no allocation and no Win32 call before them. Window class, process name and band are
//! only looked up after that, and the shared hidden set is locked only for a full match.
//!
//! The body runs inside [`contain_text`]: a panic disables the callback for good, records the
//! text for the owner and restores every hidden window at once.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::mpsc::SyncSender;
use std::time::Instant;

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Accessibility::HWINEVENTHOOK;
use windows::Win32::UI::WindowsAndMessaging::OBJID_WINDOW;

use super::query::{
    CLASS_BUF, GetWindowBand, class_name_into, is_visible, post_hide, process_name, window_band,
    window_pid,
};
use crate::flyout::{
    Match, Mode, RestoreHandle, SuppressionWindow, WindowFacts, is_host_class, should_hide,
};
use crate::guard::{FailureSlot, contain, contain_text};

/// The pid to process name cache is cleared when it grows past this (pid reuse, long runs).
const CACHE_CAP: usize = 512;

/// Everything the callback touches; lives in a thread local on the hook thread.
pub(super) struct State {
    pub(super) mode: Mode,
    pub(super) window: Arc<SuppressionWindow>,
    /// The hidden set, shared with [`RestoreHandle`] clones the owner may hold.
    pub(super) restore: RestoreHandle,
    pub(super) failure: FailureSlot,
    pub(super) names: HashMap<u32, String>,
    pub(super) band_fn: Option<GetWindowBand>,
    pub(super) matches: SyncSender<Match>,
}

impl Drop for State {
    /// Runs on the hook thread when it ends, also when it unwinds: give back what was hidden.
    fn drop(&mut self) {
        self.restore.restore_now();
    }
}

thread_local! {
    pub(super) static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
    /// Set after a contained panic; every later event returns at once.
    static DISABLED: Cell<bool> = const { Cell::new(false) };
}

impl State {
    /// Process and band lookups; called only for a window whose class already matched.
    fn facts(&mut self, hwnd: HWND, class: String) -> WindowFacts {
        let pid = window_pid(hwnd);
        if self.names.len() >= CACHE_CAP {
            self.names.clear();
        }
        let process = self
            .names
            .entry(pid)
            .or_insert_with(|| process_name(pid))
            .clone();
        WindowFacts {
            class,
            process,
            band: window_band(self.band_fn, hwnd),
        }
    }

    fn on_event(&mut self, hwnd: HWND) {
        // The class check is a stack buffer and a non-allocating compare; the process and band
        // lookups (OpenProcess, a String) only run for the few windows that pass it.
        let mut buf = [0u16; CLASS_BUF];
        let len = class_name_into(hwnd, &mut buf);
        if !is_host_class(&buf[..len]) {
            return;
        }
        let facts = self.facts(hwnd, String::from_utf16_lossy(&buf[..len]));
        if !should_hide(&facts, true) {
            return;
        }
        let key = hwnd.0 as isize;
        // The registry records the window only when it was visible just before, and journals
        // it under the same lock.
        let hidden = self.mode == Mode::Hide && self.restore.hide(key, is_visible, post_hide);
        let _ = self.matches.try_send(Match {
            hwnd: key,
            facts,
            hidden,
        });
    }
}

/// The work of one event that passed the shape and disabled checks.
fn dispatch(hwnd: HWND) {
    // `try_with`: the state has a destructor, so a late event during thread teardown finds
    // it gone; that is a no-op, never a panic.
    let _ = STATE.try_with(|cell| {
        let Ok(mut guard) = cell.try_borrow_mut() else {
            return;
        };
        if let Some(state) = guard.as_mut() {
            // The cheapest filter first: nothing below runs outside the 400 ms window.
            if state.window.is_open(Instant::now()) {
                state.on_event(hwnd);
            }
        }
    });
}

/// After a contained panic: disable the callback, record why, and give every hidden window
/// back now (the state's borrow was released by the unwind). Itself contained, so a second
/// panic here cannot unwind into Windows.
fn fail(text: String) {
    DISABLED.with(|disabled| disabled.set(true));
    let _ = contain(|| {
        let _ = STATE.try_with(|cell| {
            if let Ok(guard) = cell.try_borrow()
                && let Some(state) = guard.as_ref()
            {
                state.failure.record(text);
                state.restore.restore_now();
            }
        });
    });
}

pub(super) unsafe extern "system" fn callback(
    _hook: HWINEVENTHOOK,
    _event: u32,
    hwnd: HWND,
    id_object: i32,
    id_child: i32,
    _thread: u32,
    _time: u32,
) {
    if hwnd.0.is_null() || id_object != OBJID_WINDOW.0 || id_child != 0 {
        return;
    }
    if DISABLED.with(Cell::get) {
        return;
    }
    if let Some(text) = contain_text(|| dispatch(hwnd)) {
        fail(text);
    }
}
