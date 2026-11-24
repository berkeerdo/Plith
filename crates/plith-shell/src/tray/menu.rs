//! The popup menu of the tray icon.

use plith_core::tray::menu::{MenuLine, TrayItem};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, HMENU, MF_SEPARATOR, MF_STRING, PostMessageW,
    SetForegroundWindow, TPM_BOTTOMALIGN, TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON,
    TrackPopupMenuEx, WM_NULL,
};
use windows::core::PCWSTR;

/// Builds the menu from `lines`.
fn build(lines: &[MenuLine]) -> windows::core::Result<HMENU> {
    // SAFETY: creates an empty popup menu; the caller destroys it.
    let menu = unsafe { CreatePopupMenu() }?;
    for line in lines {
        let added = match line {
            MenuLine::Item(item, caption) => {
                let wide: Vec<u16> = caption.encode_utf16().chain(Some(0)).collect();
                // SAFETY: `menu` is the live menu above and `wide` is NUL-terminated and
                // outlives the call, which copies the text.
                unsafe {
                    AppendMenuW(
                        menu,
                        MF_STRING,
                        usize::from(item.id()),
                        PCWSTR(wide.as_ptr()),
                    )
                }
            }
            // SAFETY: `menu` is live; a separator takes no text.
            MenuLine::Separator => unsafe { AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null()) },
        };
        if let Err(error) = added {
            // SAFETY: `menu` is live and not shown.
            let _ = unsafe { DestroyMenu(menu) };
            return Err(error);
        }
    }
    Ok(menu)
}

/// Shows the menu at the screen point `at` and returns the chosen item, `None` when the menu was
/// dismissed or could not be built. The owner is made the foreground window first: without it
/// the menu does not close when the user clicks elsewhere. The `WM_NULL` afterwards ends the
/// menu's modal state (the documented tray menu recipe).
pub(super) fn track(hwnd: HWND, lines: &[MenuLine], at: (i32, i32)) -> Option<TrayItem> {
    let menu = build(lines).ok()?;
    // SAFETY: `hwnd` is the live island window of this thread and `menu` a live popup menu;
    // TPM_RETURNCMD makes the call return the chosen command id (0 when dismissed), and the
    // menu is destroyed after the call.
    let chosen = unsafe {
        let _ = SetForegroundWindow(hwnd);
        let id = TrackPopupMenuEx(
            menu,
            (TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN | TPM_NONOTIFY).0,
            at.0,
            at.1,
            hwnd,
            None,
        );
        let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyMenu(menu);
        id.0
    };
    u16::try_from(chosen).ok().and_then(TrayItem::from_id)
}
