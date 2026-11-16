//! The OSD window and the z-order band it is moved into.
//!
//! `CreateWindowInBand`, `SetWindowBand` and `GetWindowBand` are undocumented `user32` exports,
//! resolved at run time. Windows refuses the UIAccess band unless the process token carries
//! UIAccess (signed exe in a trusted folder) or the process is immersive.

use plith_core::layout::RectPx;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use windows::Win32::Foundation::{
    CloseHandle, FARPROC, HANDLE, HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM,
};
use windows::Win32::Graphics::Gdi::{
    DEVMODEW, ENUM_CURRENT_SETTINGS, EnumDisplaySettingsW, GetMonitorInfoW,
    MONITOR_DEFAULTTOPRIMARY, MONITORINFO, MonitorFromPoint,
};
use windows::Win32::Security::{GetTokenInformation, TOKEN_QUERY, TokenUIAccess};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows::Win32::System::Threading::{GetCurrentProcess, IsImmersiveProcess, OpenProcessToken};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, PostMessageW, RegisterClassExW, SW_HIDE,
    SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOZORDER, SetWindowPos, ShowWindow, ShowWindowAsync,
    WINDOW_EX_STYLE, WM_APP, WM_DISPLAYCHANGE, WM_DPICHANGED, WNDCLASSEXW, WS_EX_NOACTIVATE,
    WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};
use windows::core::{Error, PCSTR, PCWSTR, s, w};

/// `ZBID_UIACCESS`: the band above every normal and topmost window.
pub const ZBID_UIACCESS: u32 = 2;

const CLASS_NAME: PCWSTR = w!("PlithOsdBand");

/// Posted to the window by its own procedure after a display or DPI change; the UI thread's
/// message loop answers it by re-placing the window (see [`take_relayout_request`]).
pub const WM_APP_RELAYOUT: u32 = WM_APP + 2;

/// Set while a [`WM_APP_RELAYOUT`] is posted and not yet taken, so a burst of display
/// messages (a mode change often brings both) asks for one relayout.
static RELAYOUT_PENDING: AtomicBool = AtomicBool::new(false);

/// Clears the pending relayout request; returns whether one was pending. The message loop
/// calls it when it sees [`WM_APP_RELAYOUT`], before it reads the monitor again.
pub fn take_relayout_request() -> bool {
    RELAYOUT_PENDING.swap(false, Ordering::SeqCst)
}

type CreateWindowInBandFn = unsafe extern "system" fn(
    u32,
    PCWSTR,
    PCWSTR,
    u32,
    i32,
    i32,
    i32,
    i32,
    HWND,
    isize,
    HINSTANCE,
    *const core::ffi::c_void,
    u32,
) -> HWND;
type SetWindowBandFn = unsafe extern "system" fn(HWND, HWND, u32) -> windows::core::BOOL;
type GetWindowBandFn = unsafe extern "system" fn(HWND, *mut u32) -> windows::core::BOOL;

/// How the window reaches (or does not reach) the UIAccess band.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Creation {
    /// `CreateWindowInBand` with `ZBID_UIACCESS`.
    InBand,
    /// `CreateWindowExW`, then `SetWindowBand(hwnd, 0, ZBID_UIACCESS)` (the WPF path).
    SetBand,
    /// `CreateWindowExW` only.
    Plain,
}

impl Creation {
    /// The word used on the command line and in the log.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InBand => "inband",
            Self::SetBand => "setband",
            Self::Plain => "plain",
        }
    }
}

/// A shown top-level window, destroyed on drop (drop it on the thread that created it).
#[derive(Debug)]
pub struct BandWindow {
    /// The window.
    pub hwnd: HWND,
    /// The band read back with `GetWindowBand`, `None` when the export is missing or failed.
    pub band: Option<u32>,
    /// How the window was made.
    pub creation: Creation,
}

impl Drop for BandWindow {
    fn drop(&mut self) {
        // SAFETY: `hwnd` is a window this thread created and has not destroyed.
        let _ = unsafe { DestroyWindow(self.hwnd) };
    }
}

/// Looks up a `user32` export by name.
fn user32_proc(name: PCSTR) -> FARPROC {
    // SAFETY: user32 is loaded in every GUI process; `name` is a NUL-terminated string.
    unsafe {
        let module = GetModuleHandleW(w!("user32.dll")).ok()?;
        GetProcAddress(module, name)
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if msg == WM_DISPLAYCHANGE || msg == WM_DPICHANGED {
        // The work is done from the message loop, outside this (possibly sent) message.
        if !RELAYOUT_PENDING.swap(true, Ordering::SeqCst) {
            // SAFETY: posting to this live window never dereferences anything.
            let posted = unsafe { PostMessageW(Some(hwnd), WM_APP_RELAYOUT, WPARAM(0), LPARAM(0)) };
            if posted.is_err() {
                RELAYOUT_PENDING.store(false, Ordering::SeqCst);
            }
        }
        // Both messages want 0 when handled; the window is placed by the loop, not by the
        // rectangle WM_DPICHANGED suggests.
        return LRESULT(0);
    }
    // SAFETY: forwards the arguments Windows passed to this window procedure unchanged.
    unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
}

fn instance() -> Result<HINSTANCE, String> {
    // SAFETY: a null module name returns the exe's own handle.
    let module = unsafe { GetModuleHandleW(PCWSTR::null()) }.map_err(|e| e.to_string())?;
    Ok(module.into())
}

/// Registers the window class once per process.
fn register_class() -> Result<(), String> {
    static ATOM: OnceLock<u16> = OnceLock::new();
    let hinstance = instance()?;
    let atom = *ATOM.get_or_init(|| {
        let class = WNDCLASSEXW {
            cbSize: u32::try_from(size_of::<WNDCLASSEXW>()).unwrap_or(0),
            lpfnWndProc: Some(wndproc),
            hInstance: hinstance,
            lpszClassName: CLASS_NAME,
            ..Default::default()
        };
        // SAFETY: `class` is fully initialised and its strings are static.
        unsafe { RegisterClassExW(&raw const class) }
    });
    if atom == 0 {
        return Err(format!("RegisterClassExW: {}", Error::from_thread()));
    }
    Ok(())
}

fn ex_style() -> WINDOW_EX_STYLE {
    WS_EX_TOOLWINDOW
        | WS_EX_NOACTIVATE
        | WS_EX_TOPMOST
        | WS_EX_TRANSPARENT
        | WS_EX_NOREDIRECTIONBITMAP
}

fn create_in_band(rect: RectPx, hinstance: HINSTANCE) -> Result<HWND, String> {
    // SAFETY: the export has the `CreateWindowInBand` signature declared above.
    let create = user32_proc(s!("CreateWindowInBand"))
        .map(|f| unsafe { std::mem::transmute::<_, CreateWindowInBandFn>(f) })
        .ok_or("CreateWindowInBand: export missing")?;
    // SAFETY: the class is registered; every pointer is static or null.
    let hwnd = unsafe {
        create(
            ex_style().0,
            CLASS_NAME,
            w!("Plith OSD"),
            WS_POPUP.0,
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            HWND::default(),
            0,
            hinstance,
            std::ptr::null(),
            ZBID_UIACCESS,
        )
    };
    if hwnd.is_invalid() {
        return Err(format!("CreateWindowInBand: {}", Error::from_thread()));
    }
    Ok(hwnd)
}

fn create_plain(rect: RectPx, hinstance: HINSTANCE) -> Result<HWND, String> {
    // SAFETY: the class is registered; the strings are static.
    unsafe {
        CreateWindowExW(
            ex_style(),
            CLASS_NAME,
            w!("Plith OSD"),
            WS_POPUP,
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            None,
            None,
            Some(hinstance),
            None,
        )
    }
    .map_err(|e| format!("CreateWindowExW: {e}"))
}

fn set_band(hwnd: HWND) -> Result<(), String> {
    // SAFETY: the export has the `SetWindowBand` signature declared above.
    let set = user32_proc(s!("SetWindowBand"))
        .map(|f| unsafe { std::mem::transmute::<_, SetWindowBandFn>(f) })
        .ok_or("SetWindowBand: export missing")?;
    // SAFETY: `hwnd` is a live window of this thread; a null insert-after is accepted.
    if unsafe { set(hwnd, HWND::default(), ZBID_UIACCESS) }.as_bool() {
        Ok(())
    } else {
        Err(format!("SetWindowBand: {}", Error::from_thread()))
    }
}

/// Reads the window's band, `None` when the export is missing or the call fails.
#[must_use]
pub fn window_band(hwnd: HWND) -> Option<u32> {
    // SAFETY: the export has the `GetWindowBand` signature declared above.
    let get = user32_proc(s!("GetWindowBand"))
        .map(|f| unsafe { std::mem::transmute::<_, GetWindowBandFn>(f) })?;
    let mut band = 0u32;
    // SAFETY: `band` is a live local the call writes once.
    unsafe { get(hwnd, &raw mut band) }
        .as_bool()
        .then_some(band)
}

/// Creates the window at `rect` (physical pixels) the `creation` way, shows it without
/// activating it and reads its band back.
///
/// # Errors
/// Returns the failing call and its Windows error, for example `CreateWindowInBand` or
/// `SetWindowBand` refused because the process lacks UIAccess.
pub fn create_band_window(rect: RectPx, creation: Creation) -> Result<BandWindow, String> {
    register_class()?;
    let hinstance = instance()?;
    let hwnd = match creation {
        Creation::InBand => create_in_band(rect, hinstance)?,
        Creation::SetBand | Creation::Plain => create_plain(rect, hinstance)?,
    };
    let mut window = BandWindow {
        hwnd,
        band: None,
        creation,
    };
    if creation == Creation::SetBand {
        set_band(hwnd)?;
    }
    // SAFETY: `hwnd` is a live window of this thread.
    let _ = unsafe { ShowWindow(hwnd, SW_SHOWNOACTIVATE) };
    window.band = window_band(hwnd);
    Ok(window)
}

/// Centres a `width` x `height` DIP rectangle at the top of `monitor`, in physical pixels at
/// `scale` pixels per DIP.
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn top_centre(monitor: RectPx, scale: f64, width: f64, height: f64) -> RectPx {
    let w = (width * scale).round() as i32;
    let h = (height * scale).round() as i32;
    RectPx {
        x: monitor.x + (monitor.width - w) / 2,
        y: monitor.y,
        width: w,
        height: h,
    }
}

/// Moves and sizes `hwnd` to `rect` (physical pixels) without activating it, changing its
/// z-order or band, or changing whether it is shown.
///
/// # Errors
/// Returns the `SetWindowPos` error.
pub fn move_window(hwnd: HWND, rect: RectPx) -> Result<(), String> {
    // SAFETY: `hwnd` is a live window of this thread; no pointer is passed.
    unsafe {
        SetWindowPos(
            hwnd,
            None,
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            SWP_NOACTIVATE | SWP_NOZORDER,
        )
    }
    .map_err(|e| format!("SetWindowPos: {e}"))
}

/// The primary monitor's rectangle and its scale (pixels per DIP).
///
/// # Errors
/// Returns the failing call.
pub fn primary_monitor() -> Result<(RectPx, f64), String> {
    // SAFETY: plain queries; `info` and the DPI outputs are live locals.
    unsafe {
        let monitor = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
        let mut info = MONITORINFO {
            cbSize: u32::try_from(size_of::<MONITORINFO>()).unwrap_or(0),
            ..Default::default()
        };
        if !GetMonitorInfoW(monitor, &raw mut info).as_bool() {
            return Err(String::from("GetMonitorInfoW failed"));
        }
        let (mut dpi_x, mut dpi_y) = (96u32, 96u32);
        GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &raw mut dpi_x, &raw mut dpi_y)
            .map_err(|e| format!("GetDpiForMonitor: {e}"))?;
        let r = info.rcMonitor;
        let rect = RectPx {
            x: r.left,
            y: r.top,
            width: r.right - r.left,
            height: r.bottom - r.top,
        };
        Ok((rect, f64::from(dpi_x) / 96.0))
    }
}

/// Refresh rate of the primary display in hertz, 60 when the query fails.
#[must_use]
pub fn refresh_hz() -> f64 {
    let mut mode = DEVMODEW {
        dmSize: u16::try_from(size_of::<DEVMODEW>()).unwrap_or(0),
        ..DEVMODEW::default()
    };
    // SAFETY: a null device name means the primary display; `mode` is a live DEVMODEW with
    // `dmSize` set.
    let ok = unsafe { EnumDisplaySettingsW(PCWSTR::null(), ENUM_CURRENT_SETTINGS, &raw mut mode) };
    // 0 and 1 mean "hardware default" rather than a rate.
    if ok.as_bool() && mode.dmDisplayFrequency > 1 {
        f64::from(mode.dmDisplayFrequency)
    } else {
        60.0
    }
}

/// Shows (without activating) or hides `hwnd` from any thread: the window's own thread does
/// it when it next pumps messages.
pub fn set_visible(hwnd: HWND, visible: bool) {
    let command = if visible { SW_SHOWNOACTIVATE } else { SW_HIDE };
    // SAFETY: `hwnd` is a live window; ShowWindowAsync only posts to its thread.
    let _ = unsafe { ShowWindowAsync(hwnd, command) };
}

/// Whether Windows treats this process as immersive.
#[must_use]
pub fn is_immersive() -> bool {
    // SAFETY: the pseudo handle of the current process is always valid.
    unsafe { IsImmersiveProcess(GetCurrentProcess()) }.is_ok()
}

/// The `TokenUIAccess` flag of this process token.
///
/// # Errors
/// Returns the failing call.
pub fn token_ui_access() -> Result<bool, String> {
    let mut token = HANDLE::default();
    let mut flag = 0u32;
    let mut len = 0u32;
    // SAFETY: the out pointers are live locals; the token is closed on every path below.
    unsafe {
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &raw mut token)
            .map_err(|e| format!("OpenProcessToken: {e}"))?;
        let read = GetTokenInformation(
            token,
            TokenUIAccess,
            Some((&raw mut flag).cast()),
            u32::try_from(size_of::<u32>()).unwrap_or(4),
            &raw mut len,
        );
        let _ = CloseHandle(token);
        read.map_err(|e| format!("GetTokenInformation: {e}"))?;
    }
    Ok(flag != 0)
}

#[cfg(test)]
mod tests;
