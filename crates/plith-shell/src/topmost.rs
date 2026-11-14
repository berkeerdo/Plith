//! The event-driven topmost fight: a `WinEvent` hook reports every foreground change, the shell
//! raises its window at once (unless a raise just happened) and again on a retry timer (see
//! `plith_core::topmost`).

use crate::handles::hwnd_of;
use crate::window::state::{Inner, current};
use crate::window::{ShellEvent, guard};
use plith_core::topmost::TopmostAction;
use windows::Win32::Foundation::{E_FAIL, HWND};
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows::Win32::UI::WindowsAndMessaging::{
    EVENT_SYSTEM_FOREGROUND, GW_HWNDPREV, GetClassNameW, GetWindow, HWND_TOPMOST, IsWindowVisible,
    KillTimer, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetTimer, SetWindowPos,
    WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS,
};

/// Timer id of the retry timer, owned by the visual window.
pub(crate) const RETRY_TIMER_ID: usize = 1;

/// How many windows above us to look through when naming the culprit.
const CULPRIT_SEARCH_DEPTH: usize = 32;

/// Class name logged when the foreground window is already gone.
const UNKNOWN_CLASS: &str = "?";

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
        // SetWinEventHook documents no GetLastError, so the thread error may be stale.
        return Err(windows::core::Error::from(E_FAIL));
    }
    Ok(hook.0 as isize)
}

/// Removes the foreground hook.
pub(crate) fn remove_hook(hook: isize) {
    // SAFETY: `hook` came from `SetWinEventHook` on this thread and is removed once, so it is a live hook handle.
    let _ = unsafe { UnhookWinEvent(HWINEVENTHOOK(hook as *mut core::ffi::c_void)) };
}

unsafe extern "system" fn foreground_proc(
    _hook: HWINEVENTHOOK,
    _event: u32,
    hwnd: HWND,
    _object: i32,
    _child: i32,
    _thread: u32,
    _time: u32,
) {
    if let Some(inner) = current()
        && let Err(message) = guard("foreground hook", || on_foreground(&inner, hwnd))
    {
        inner.fail(message);
    }
}

/// The window class of `hwnd`, or `?` when the window is gone.
fn class_name(hwnd: HWND) -> String {
    let mut buffer = [0_u16; 256];
    // SAFETY: the buffer is writable for the call and its length comes from the slice, so the
    // API cannot write past it; a stale handle only makes the call return 0.
    let len = unsafe { GetClassNameW(hwnd, &mut buffer) };
    let name = String::from_utf16_lossy(&buffer[..usize::try_from(len).unwrap_or(0)]);
    if name.is_empty() {
        UNKNOWN_CLASS.to_string()
    } else {
        name
    }
}

/// The class of the nearest visible window above ours that is not ours. Invisible windows (the
/// default IME window, message windows) cannot cover the island, so the walk goes past them.
fn find_culprit(inner: &Inner) -> Option<String> {
    let ours = inner.windows();
    let mut walker = hwnd_of(inner.visual.get());
    for _ in 0..CULPRIT_SEARCH_DEPTH {
        // SAFETY: GetWindow reads no memory of ours and accepts any handle value; a stale one fails.
        match unsafe { GetWindow(walker, GW_HWNDPREV) } {
            Ok(above) if !above.0.is_null() => {
                // SAFETY: IsWindowVisible reads no memory of ours and accepts any handle value.
                let visible = unsafe { IsWindowVisible(above) }.as_bool();
                if visible && !ours.contains(&(above.0 as isize)) {
                    return Some(class_name(above));
                }
                walker = above;
            }
            _ => break,
        }
    }
    None
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

fn arm_timer(inner: &Inner, ms: u32) {
    // SAFETY: the visual window belongs to this thread; no callback is used, so the timer
    // arrives as WM_TIMER.
    let _ = unsafe { SetTimer(Some(hwnd_of(inner.visual.get())), RETRY_TIMER_ID, ms, None) };
}

/// A foreground change: raise now (unless one just happened), restart the retry timer and tell
/// the app who is in the foreground and who sat above us.
pub(crate) fn on_foreground(inner: &Inner, foreground: HWND) {
    kill_timer(inner);
    let now_ms = u64::try_from(inner.origin.elapsed().as_millis()).unwrap_or(u64::MAX);
    let mut fight = inner.fight.get();
    let action = fight.on_foreground(now_ms);
    inner.fight.set(fight);
    let TopmostAction::Raise {
        retry_in_ms,
        immediate,
    } = action
    else {
        return;
    };
    let above = if immediate {
        let above = find_culprit(inner);
        raise(inner);
        inner.culprit.borrow_mut().clone_from(&above);
        above
    } else {
        None
    };
    if let Some(ms) = retry_in_ms {
        arm_timer(inner, ms);
    }
    inner.emit(ShellEvent::Raised {
        foreground: class_name(foreground),
        above,
        immediate,
    });
}

/// The retry timer fired.
pub(crate) fn on_timer(inner: &Inner) {
    kill_timer(inner);
    let mut fight = inner.fight.get();
    let action = fight.on_timer();
    inner.fight.set(fight);
    let TopmostAction::Raise { retry_in_ms, .. } = action else {
        return;
    };
    raise(inner);
    if let Some(ms) = retry_in_ms {
        arm_timer(inner, ms);
    }
}

/// Stops the retry timer.
pub(crate) fn kill_timer(inner: &Inner) {
    // SAFETY: the visual window belongs to this thread, so KillTimer's handle is valid; failing because no timer runs is fine.
    let _ = unsafe { KillTimer(Some(hwnd_of(inner.visual.get())), RETRY_TIMER_ID) };
}
