//! The `Shell_NotifyIconW` calls: add with version 4, remove, and the icon from the exe.

use super::decode::fill_tip;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi};
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_SETVERSION,
    NOTIFYICON_VERSION_4, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    HICON, IDI_APPLICATION, IMAGE_ICON, LR_SHARED, LoadIconW, LoadImageW, SM_CXSMICON,
};
use windows::core::{PCWSTR, Result};

/// The one icon of the app; `NIM_*` calls name it by window and this id.
const ICON_ID: u32 = 1;

/// The resource id the build script gives the exe's icon.
const ICON_RESOURCE: usize = 1;

fn data(hwnd: HWND) -> NOTIFYICONDATAW {
    NOTIFYICONDATAW {
        cbSize: u32::try_from(size_of::<NOTIFYICONDATAW>()).unwrap_or(0),
        hWnd: hwnd,
        uID: ICON_ID,
        ..NOTIFYICONDATAW::default()
    }
}

/// The small icon from the exe's resources at the window's DPI; the stock application icon when
/// the exe carries none (a test binary, a build without the resource).
fn icon(hwnd: HWND) -> HICON {
    // SAFETY: GetDpiForWindow takes any handle (0 on a bad one), the metric call has no pointer
    // input, a null module name asks for the running exe and the resource id is the integer form
    // of a name; LR_SHARED images are owned by the system and must not be destroyed.
    unsafe {
        let dpi = GetDpiForWindow(hwnd).max(96);
        let size = GetSystemMetricsForDpi(SM_CXSMICON, dpi);
        if let Ok(module) = GetModuleHandleW(None)
            && let Ok(handle) = LoadImageW(
                Some(module.into()),
                PCWSTR(ICON_RESOURCE as *const u16),
                IMAGE_ICON,
                size,
                size,
                LR_SHARED,
            )
        {
            return HICON(handle.0);
        }
        LoadIconW(None, IDI_APPLICATION).unwrap_or_default()
    }
}

/// Adds the icon with `tip`, then switches the callbacks to version 4.
pub(super) fn add(hwnd: HWND, callback: u32, tip: &str) -> Result<()> {
    let mut nid = data(hwnd);
    nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_SHOWTIP;
    nid.uCallbackMessage = callback;
    nid.hIcon = icon(hwnd);
    nid.szTip = fill_tip(tip);
    // SAFETY: `nid` is fully initialised with its size set, and lives across both calls.
    unsafe {
        Shell_NotifyIconW(NIM_ADD, &raw const nid).ok()?;
        nid.Anonymous.uVersion = NOTIFYICON_VERSION_4;
        Shell_NotifyIconW(NIM_SETVERSION, &raw const nid).ok()
    }
}

/// Removes the icon. A failure (Explorer already gone) leaves nothing to do.
pub(super) fn remove(hwnd: HWND) {
    let nid = data(hwnd);
    // SAFETY: `nid` is initialised with its size set.
    let _ = unsafe { Shell_NotifyIconW(NIM_DELETE, &raw const nid) };
}
