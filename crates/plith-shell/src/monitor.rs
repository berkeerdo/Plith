//! The primary monitor: rectangle, work area, DPI scale and refresh rate.

use plith_core::layout::RectPx;
use windows::Win32::Foundation::POINT;
use windows::Win32::Graphics::Gdi::{
    DEVMODEW, ENUM_CURRENT_SETTINGS, EnumDisplaySettingsW, GetMonitorInfoW,
    MONITOR_DEFAULTTOPRIMARY, MONITORINFO, MONITORINFOEXW, MonitorFromPoint,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN, SM_REMOTESESSION,
};
use windows::core::PCWSTR;

/// What the shell and the renderer need to know about a monitor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MonitorInfo {
    /// The `HMONITOR` as an integer, for the render thread.
    pub handle: isize,
    /// Monitor rectangle in physical pixels.
    pub rect: RectPx,
    /// Top of the work area in physical pixels.
    pub work_top_px: i32,
    /// Pixels per DIP.
    pub scale: f64,
    /// Nominal refresh rate in hertz.
    pub hz: f64,
}

const FALLBACK_HZ: f64 = 60.0;

fn refresh_rate(device: &[u16; 32]) -> f64 {
    let mut mode = DEVMODEW {
        dmSize: u16::try_from(size_of::<DEVMODEW>()).unwrap_or(0),
        ..DEVMODEW::default()
    };
    // SAFETY: `device` is a NUL-terminated name from `GetMonitorInfoW` and `mode` is a valid
    // DEVMODEW with `dmSize` set, both alive for the call.
    let ok = unsafe {
        EnumDisplaySettingsW(
            PCWSTR(device.as_ptr()),
            ENUM_CURRENT_SETTINGS,
            &raw mut mode,
        )
    };
    // 0 and 1 mean "hardware default" rather than a rate.
    if ok.as_bool() && mode.dmDisplayFrequency > 1 {
        f64::from(mode.dmDisplayFrequency)
    } else {
        FALLBACK_HZ
    }
}

/// Describes the primary monitor, falling back to the screen metrics, 96 DPI and 60 Hz when a
/// query fails.
#[must_use]
pub fn primary_monitor() -> MonitorInfo {
    // SAFETY: the point is passed by value and the flag asks for the primary monitor.
    let hmon = unsafe { MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY) };
    let mut info = MONITORINFOEXW::default();
    info.monitorInfo.cbSize = u32::try_from(size_of::<MONITORINFOEXW>()).unwrap_or(0);
    // SAFETY: `info` is a MONITORINFOEXW whose first member is the MONITORINFO that the call
    // fills; `cbSize` announces the extended size.
    let got = unsafe { GetMonitorInfoW(hmon, (&raw mut info).cast::<MONITORINFO>()) };
    let (rect, work_top_px) = if got.as_bool() {
        let m = info.monitorInfo.rcMonitor;
        (
            RectPx {
                x: m.left,
                y: m.top,
                width: m.right - m.left,
                height: m.bottom - m.top,
            },
            info.monitorInfo.rcWork.top,
        )
    } else {
        // SAFETY: plain metric queries.
        let (w, h) = unsafe { (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN)) };
        (
            RectPx {
                x: 0,
                y: 0,
                width: w,
                height: h,
            },
            0,
        )
    };
    let (mut dpi_x, mut dpi_y) = (96_u32, 96_u32);
    // SAFETY: both out pointers are valid for the call. On failure they keep 96.
    let _ = unsafe { GetDpiForMonitor(hmon, MDT_EFFECTIVE_DPI, &raw mut dpi_x, &raw mut dpi_y) };
    MonitorInfo {
        handle: hmon.0 as isize,
        rect,
        work_top_px,
        scale: f64::from(dpi_x) / 96.0,
        hz: refresh_rate(&info.szDevice),
    }
}

/// True when the process runs in a remote desktop session, where Windows turns animation off.
#[must_use]
pub fn is_remote_session() -> bool {
    // SAFETY: a plain metric query.
    unsafe { GetSystemMetrics(SM_REMOTESESSION) != 0 }
}
