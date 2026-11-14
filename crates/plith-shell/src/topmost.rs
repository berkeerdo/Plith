//! The event-driven topmost fight: a `WinEvent` hook reports every foreground change, the shell
//! raises its windows at once and again on a retry timer (see `plith_core::topmost`).

use crate::window::state::{Inner, current};
use plith_core::topmost::TopmostAction;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows::Win32::UI::WindowsAndMessaging::{
    EVENT_SYSTEM_FOREGROUND, GW_HWNDPREV, GetClassNameW, GetWindow, HWND_TOPMOST, KillTimer,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetTimer, SetWindowPos, WINEVENT_OUTOFCONTEXT,
    WINEVENT_SKIPOWNPROCESS,
};

/// Timer id of the retry timer, owned by the visual window.
pub(crate) const RETRY_TIMER_ID: usize = 1;

/// How many windows above us to look through when naming the culprit.
const CULPRIT_SEARCH_DEPTH: usize = 32;

fn hwnd_of(raw: isize) -> HWND {
    HWND(raw as *mut core::ffi::c_void)
}

/// Installs the foreground hook. It reaches the shell state through the thread-local because a
/// `WinEvent` procedure has no user pointer; it is called on this thread from its message loop.
pub(crate) fn install_hook() -> windows::core::Result<isize> {
    // SAFETY: `foreground_proc` matches WINEVENTPROC and lives for the whole process. The hook
    // is out of context, so it runs on this thread, and is removed in `remove_hook`.
    let hook = unsafe {
        SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            None,
            Some(foreground_proc),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        )
    };
    if hook.0.is_null() {
        return Err(windows::core::Error::from_thread());
    }
    Ok(hook.0 as isize)
}

/// Removes the foreground hook.
pub(crate) fn remove_hook(hook: isize) {
    // SAFETY: `hook` came from `SetWinEventHook` on this thread and is removed once.
    let _ = unsafe { UnhookWinEvent(HWINEVENTHOOK(hook as *mut core::ffi::c_void)) };
}

unsafe extern "system" fn foreground_proc(
    _hook: HWINEVENTHOOK,
    _event: u32,
    _hwnd: HWND,
    _object: i32,
    _child: i32,
    _thread: u32,
    _time: u32,
) {
    if let Some(inner) = current() {
        on_foreground(&inner);
    }
}

fn class_name(hwnd: HWND) -> String {
    let mut buffer = [0_u16; 256];
    // SAFETY: the buffer is valid for the call and the length comes from its slice.
    let len = unsafe { GetClassNameW(hwnd, &mut buffer) };
    String::from_utf16_lossy(&buffer[..usize::try_from(len).unwrap_or(0)])
}

/// Records the class of the nearest window above ours that is not ours.
fn record_culprit(inner: &Inner) {
    let ours = inner.windows();
    let mut walker = hwnd_of(inner.visual.get());
    let mut culprit = None;
    for _ in 0..CULPRIT_SEARCH_DEPTH {
        // SAFETY: GetWindow accepts any handle value and fails for a stale one.
        match unsafe { GetWindow(walker, GW_HWNDPREV) } {
            Ok(above) if !above.0.is_null() => {
                if !ours.contains(&(above.0 as isize)) {
                    culprit = Some(class_name(above));
                    break;
                }
                walker = above;
            }
            _ => break,
        }
    }
    *inner.culprit.borrow_mut() = culprit;
}

fn raise(inner: &Inner) {
    for hwnd in inner.windows() {
        // SAFETY: the handles are windows of this thread; the flags make the call a pure
        // z-order change that neither moves, resizes nor activates.
        let _ = unsafe {
            SetWindowPos(
                hwnd_of(hwnd),
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            )
        };
    }
}

fn apply(inner: &Inner, action: TopmostAction) {
    let TopmostAction::Raise { retry_in_ms } = action else {
        return;
    };
    raise(inner);
    if let Some(ms) = retry_in_ms {
        // SAFETY: the visual window belongs to this thread; no callback is used, so the timer
        // arrives as WM_TIMER.
        let _ = unsafe { SetTimer(Some(hwnd_of(inner.visual.get())), RETRY_TIMER_ID, ms, None) };
    }
}

/// A foreground change: log who sits above, raise now and restart the retry timer.
pub(crate) fn on_foreground(inner: &Inner) {
    kill_timer(inner);
    record_culprit(inner);
    let mut fight = inner.fight.get();
    let action = fight.on_foreground();
    inner.fight.set(fight);
    apply(inner, action);
}

/// The retry timer fired.
pub(crate) fn on_timer(inner: &Inner) {
    kill_timer(inner);
    let mut fight = inner.fight.get();
    let action = fight.on_timer();
    inner.fight.set(fight);
    apply(inner, action);
}

/// Stops the retry timer.
pub(crate) fn kill_timer(inner: &Inner) {
    // SAFETY: the visual window belongs to this thread; failing because no timer runs is fine.
    let _ = unsafe { KillTimer(Some(hwnd_of(inner.visual.get())), RETRY_TIMER_ID) };
}
