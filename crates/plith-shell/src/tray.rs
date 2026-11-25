//! The notification-area icon: version 4 callbacks on the island's own window, a popup menu
//! and the re-add after an Explorer restart.
//!
//! Everything runs on the UI thread inside the island's window procedure: no thread and no
//! timer. The icon sends its mouse events as `WM_APP_TRAY` to the island window; a left click
//! and the menu's commands leave as `ShellEvent::Tray`. The captions come from the app
//! (`plith_core::tray::menu`) and are replaced through a [`TrayHandle`] when the language changes.

mod decode;
mod menu;
mod notify;

use crate::handles::hwnd_of;
use crate::window::Shell;
use crate::window::ShellEvent;
use crate::window::state::{Inner, current};
use plith_core::tray::TOOLTIP;
use plith_core::tray::menu::{MenuLine, TrayItem};
use std::marker::PhantomData;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{RegisterWindowMessageW, WM_APP};
use windows::core::w;

/// The icon's callback message. `WM_APP + 1` to `+4` are taken (see `poster` and `display`).
pub(crate) const WM_APP_TRAY: u32 = WM_APP + 5;

/// What the shell keeps of the tray icon.
#[derive(Debug, Default)]
pub(crate) struct TrayState {
    /// The menu, in order, with its captions.
    pub lines: Vec<MenuLine>,
    /// The id of the `TaskbarCreated` message, 0 until the icon was asked for.
    pub created_msg: u32,
    /// Whether the taskbar holds the icon now.
    pub added: bool,
}

impl Shell {
    /// Puts the Plith icon in the notification area with the menu `lines`. When Explorer is not
    /// ready (sign in) the call fails, and the icon still arrives with the `TaskbarCreated`
    /// message Explorer sends once its taskbar exists.
    ///
    /// # Errors
    /// Returns the Win32 error when the taskbar refuses the icon.
    pub fn add_tray(&self, lines: &[MenuLine]) -> windows::core::Result<()> {
        let inner = &self.inner;
        let hwnd = hwnd_of(inner.visual.get());
        {
            let mut tray = inner.tray.borrow_mut();
            // SAFETY: the name is a static NUL-terminated string; the call has no other input.
            tray.created_msg = unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) };
            tray.lines = lines.to_vec();
        }
        add(inner, hwnd)
    }

    /// A handle that replaces the menu captions, from the UI thread.
    #[must_use]
    pub fn tray_handle(&self) -> TrayHandle {
        TrayHandle {
            _ui_thread: PhantomData,
        }
    }
}

/// Replaces the tray menu's captions (a language change). It lives on the UI thread.
#[derive(Debug, Clone, Copy)]
pub struct TrayHandle {
    _ui_thread: PhantomData<*const ()>,
}

impl TrayHandle {
    /// The menu to show from now on.
    pub fn set_menu(&self, lines: &[MenuLine]) {
        if let Some(inner) = current() {
            inner.tray.borrow_mut().lines = lines.to_vec();
        }
    }

    /// Shows a balloon with `title` and `body` over the icon. A click on it arrives as
    /// `ShellEvent::TrayNotice`. Windows may hold the balloon back (focus assist); that is its
    /// call, and a failure leaves nothing to do.
    pub fn notice(&self, title: &str, body: &str) {
        if let Some(inner) = current() {
            let hwnd = hwnd_of(inner.visual.get());
            if inner.tray.borrow().added {
                notify::balloon(hwnd, title, body);
            }
        }
    }
}

/// Adds the icon and records it.
fn add(inner: &Inner, hwnd: HWND) -> windows::core::Result<()> {
    notify::add(hwnd, WM_APP_TRAY, TOOLTIP)?;
    inner.tray.borrow_mut().added = true;
    Ok(())
}

/// Removes the icon, if it is there. Called when the window is destroyed.
pub(crate) fn remove(inner: &Inner, hwnd: HWND) {
    if std::mem::take(&mut inner.tray.borrow_mut().added) {
        notify::remove(hwnd);
    }
}

/// Whether `msg` is the broadcast Explorer sends when its taskbar is created again.
pub(crate) fn is_taskbar_created(inner: &Inner, msg: u32) -> bool {
    let id = inner.tray.borrow().created_msg;
    id != 0 && msg == id
}

/// The taskbar was created again (an Explorer restart, or its first start at sign in): the old
/// icon is gone, so add it anew.
pub(crate) fn on_taskbar_created(inner: &Inner, hwnd: HWND) {
    let _ = add(inner, hwnd);
}

/// The icon's callback: a click opens the settings, a context request shows the menu.
pub(crate) fn on_callback(inner: &Inner, hwnd: HWND, wparam: WPARAM, lparam: LPARAM) {
    match decode::click_of(decode::event_of(lparam.0)) {
        decode::Click::Open => inner.emit(ShellEvent::Tray(TrayItem::Settings)),
        decode::Click::Notice => inner.emit(ShellEvent::TrayNotice),
        decode::Click::Menu => {
            let lines = inner.tray.borrow().lines.clone();
            let at = decode::anchor_of(wparam.0);
            if let Some(item) = menu::track(hwnd, &lines, at) {
                inner.emit(ShellEvent::Tray(item));
            }
        }
        decode::Click::Ignore => {}
    }
}
