//! Event-driven suppression signals: display on/off, session lock and fullscreen apps. Nothing
//! here polls; each signal arrives as a window message.

use crate::poster::WM_APP_BAR;
use crate::window::ShellEvent;
use crate::window::state::Inner;
use plith_core::scene::Suppression;
use windows::Win32::Foundation::{HANDLE, HWND, LPARAM, WPARAM};
use windows::Win32::System::Power::{
    HPOWERNOTIFY, POWERBROADCAST_SETTING, RegisterPowerSettingNotification,
    UnregisterPowerSettingNotification,
};
use windows::Win32::System::RemoteDesktop::{
    NOTIFY_FOR_THIS_SESSION, WTSRegisterSessionNotification, WTSUnRegisterSessionNotification,
};
use windows::Win32::System::SystemServices::GUID_SESSION_DISPLAY_STATUS;
use windows::Win32::UI::Shell::{
    ABM_NEW, ABM_REMOVE, ABN_FULLSCREENAPP, APPBARDATA, SHAppBarMessage,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DEVICE_NOTIFY_WINDOW_HANDLE, PBT_POWERSETTINGCHANGE, WTS_SESSION_LOCK, WTS_SESSION_UNLOCK,
};
use windows::core::GUID;

fn appbar_data(hwnd: HWND) -> APPBARDATA {
    APPBARDATA {
        cbSize: u32::try_from(size_of::<APPBARDATA>()).unwrap_or(0),
        hWnd: hwnd,
        uCallbackMessage: WM_APP_BAR,
        ..APPBARDATA::default()
    }
}

/// Registers for display state, session changes and fullscreen notifications on `hwnd`.
pub(crate) fn register(inner: &Inner, hwnd: HWND) -> windows::core::Result<()> {
    let mut regs = inner.registrations.borrow_mut();
    // SAFETY: `hwnd` is a live window of this thread and the GUID constant outlives the call.
    let power = unsafe {
        RegisterPowerSettingNotification(
            HANDLE(hwnd.0),
            &GUID_SESSION_DISPLAY_STATUS,
            DEVICE_NOTIFY_WINDOW_HANDLE,
        )
    }?;
    regs.power = Some(power.0);
    // SAFETY: `hwnd` is a live window of this thread.
    unsafe { WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION) }?;
    regs.session = true;
    let mut data = appbar_data(hwnd);
    // SAFETY: `data` is a valid APPBARDATA for the call. Only ABM_NEW is sent, never ABM_SETPOS,
    // so no screen space is reserved.
    regs.appbar = unsafe { SHAppBarMessage(ABM_NEW, &raw mut data) } != 0;
    Ok(())
}

/// Releases everything `register` acquired.
pub(crate) fn unregister(inner: &Inner, hwnd: HWND) {
    let mut regs = inner.registrations.borrow_mut();
    if let Some(power) = regs.power.take() {
        // SAFETY: the handle came from RegisterPowerSettingNotification and is released once.
        let _ = unsafe { UnregisterPowerSettingNotification(HPOWERNOTIFY(power)) };
    }
    if std::mem::take(&mut regs.session) {
        // SAFETY: the window was registered in `register`.
        let _ = unsafe { WTSUnRegisterSessionNotification(hwnd) };
    }
    if std::mem::take(&mut regs.appbar) {
        let mut data = appbar_data(hwnd);
        // SAFETY: `data` is a valid APPBARDATA; the window was registered with ABM_NEW.
        let _ = unsafe { SHAppBarMessage(ABM_REMOVE, &raw mut data) };
    }
}

fn update(inner: &Inner, change: impl FnOnce(&mut Suppression)) {
    let mut next = inner.suppression.get();
    change(&mut next);
    if next != inner.suppression.get() {
        inner.suppression.set(next);
        inner.emit(ShellEvent::Suppression(next));
    }
}

/// `WM_POWERBROADCAST`: tracks the display state of this session (0 off, 1 on, 2 dimmed).
pub(crate) fn on_power(inner: &Inner, wparam: WPARAM, lparam: LPARAM) {
    if u32::try_from(wparam.0).ok() != Some(PBT_POWERSETTINGCHANGE) || lparam.0 == 0 {
        return;
    }
    // SAFETY: for PBT_POWERSETTINGCHANGE, lParam points at a POWERBROADCAST_SETTING that stays
    // valid while the message is handled.
    let setting = unsafe { &*(lparam.0 as *const POWERBROADCAST_SETTING) };
    let state = if setting.DataLength >= 4 {
        // SAFETY: `DataLength` is at least 4, so a DWORD follows the header; it may be unaligned.
        unsafe { std::ptr::read_unaligned(setting.Data.as_ptr().cast::<u32>()) }
    } else {
        0
    };
    if let Some(off) = display_off(&setting.PowerSetting, setting.DataLength, state) {
        update(inner, |s| s.display_off = off);
    }
}

/// Whether a power setting change turns the session display off (`Some(true)`) or on. Only
/// the session display counts: the console display sleeps while an RDP session is on screen.
fn display_off(setting: &GUID, len: u32, state: u32) -> Option<bool> {
    (*setting == GUID_SESSION_DISPLAY_STATUS && len >= 4).then_some(state == 0)
}

/// `WM_WTSSESSION_CHANGE`: tracks lock and unlock.
pub(crate) fn on_session(inner: &Inner, wparam: WPARAM) {
    match u32::try_from(wparam.0) {
        Ok(WTS_SESSION_LOCK) => update(inner, |s| s.locked = true),
        Ok(WTS_SESSION_UNLOCK) => update(inner, |s| s.locked = false),
        _ => {}
    }
}

/// The `AppBar` callback: `ABN_FULLSCREENAPP` with a non-zero `lParam` means a fullscreen app opened.
pub(crate) fn on_appbar(inner: &Inner, wparam: WPARAM, lparam: LPARAM) {
    if u32::try_from(wparam.0).ok() == Some(ABN_FULLSCREENAPP) {
        update(inner, |s| s.fullscreen = lparam.0 != 0);
    }
}

#[cfg(test)]
mod tests;
