//! The settings window: a normal top-level window with its own class and window procedure.
//!
//! It draws nothing. The render thread presents to its `HWND` (`plith_render::sheet`); this module
//! turns Win32 input into [`SheetEvent`]s in DIP and keeps the window's size and DPI honest.

mod clipboard;
mod keys;
mod proc;
#[cfg(test)]
mod tests;

use crate::handles::hwnd_of;
use plith_core::sheet::SheetKey;
use proc::{State, WM_APP_FLUSH, client_metrics, destroy, window_proc};
use std::ffi::c_void;
use std::rc::Rc;
use windows::Win32::Foundation::{
    ERROR_CLASS_ALREADY_EXISTS, GetLastError, HINSTANCE, HWND, LPARAM, POINT, RECT, WPARAM,
};
use windows::Win32::Graphics::Dwm::{DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, HMONITOR, MONITOR_DEFAULTTOPRIMARY, MONITORINFO, MonitorFromPoint,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{AdjustWindowRectExForDpi, GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, FLASHW_ALL, FLASHW_TIMERNOFG, FLASHWINFO, FlashWindowEx, IDC_ARROW, IsIconic,
    LoadCursorW, PostMessageW, RegisterClassExW, SW_RESTORE, SW_SHOW, SetForegroundWindow,
    SetWindowTextW, ShowWindow, WINDOW_EX_STYLE, WINDOW_STYLE, WNDCLASSEXW, WS_CAPTION,
    WS_EX_NOREDIRECTIONBITMAP, WS_MINIMIZEBOX, WS_OVERLAPPED, WS_SYSMENU,
};
use windows::core::{PCWSTR, w};

pub use clipboard::set_clipboard;

/// Why the settings window could not be created.
pub type ShellError = windows::core::Error;

const CLASS: PCWSTR = w!("Plith.Settings");
/// Client size in DIP, as drawn.
const CLIENT_DIP: (u32, u32) = (840, 640);
const STYLE: WINDOW_STYLE =
    WINDOW_STYLE(WS_OVERLAPPED.0 | WS_CAPTION.0 | WS_SYSMENU.0 | WS_MINIMIZEBOX.0);
const EX_STYLE: WINDOW_EX_STYLE = WS_EX_NOREDIRECTIONBITMAP;

/// Something that happened in the settings window. Coordinates are DIP relative to the client
/// area's top-left.
#[derive(Debug, Clone, PartialEq)]
pub enum SheetEvent {
    /// The pointer moved to `(x, y)`.
    Move(f64, f64),
    /// The pointer left the client area.
    Leave,
    /// The left button went down at `(x, y)`.
    Click(f64, f64),
    /// A key the sheet understands.
    Key(SheetKey),
    /// The client area has a new size or scale (pixels per DIP).
    Resized {
        /// Client width in pixels.
        width_px: u32,
        /// Client height in pixels.
        height_px: u32,
        /// Pixels per DIP.
        scale: f64,
    },
    /// The window was closed (caption button, Alt+F4) and is destroyed.
    Closed,
}

/// Handle to the settings window. It lives on the UI thread and is not `Send`. Dropping it
/// destroys the window without sending [`SheetEvent::Closed`].
pub struct SheetWindow {
    hwnd: isize,
    state: Rc<State>,
}

fn register_class(instance: HINSTANCE) -> windows::core::Result<()> {
    // SAFETY: the stock arrow cursor is a system resource, so a null instance and a predefined id are valid.
    let cursor = unsafe { LoadCursorW(None, IDC_ARROW) }?;
    let class = WNDCLASSEXW {
        cbSize: u32::try_from(size_of::<WNDCLASSEXW>()).unwrap_or(0),
        lpfnWndProc: Some(window_proc),
        hInstance: instance,
        hCursor: cursor,
        lpszClassName: CLASS,
        ..WNDCLASSEXW::default()
    };
    // SAFETY: `class` is fully initialised and the class name is a static NUL-terminated string.
    let atom = unsafe { RegisterClassExW(&raw const class) };
    // SAFETY: GetLastError has no preconditions; RegisterClassExW just failed on this thread.
    if atom == 0 && unsafe { GetLastError() } != ERROR_CLASS_ALREADY_EXISTS {
        return Err(windows::core::Error::from_thread());
    }
    Ok(())
}

/// `dip` device-independent pixels at `dpi`, rounded to the nearest pixel.
fn scaled(dip: u32, dpi: u32) -> i32 {
    i32::try_from((u64::from(dip) * u64::from(dpi) + 48) / 96).unwrap_or(i32::MAX)
}

/// Outer window rectangle in pixels and the scale: the 840 x 640 DIP client area at `monitor`'s
/// scale, centered on its work area.
fn placement(monitor: isize) -> (RECT, f64) {
    let mut hmon = HMONITOR(monitor as *mut c_void);
    let mut info = MONITORINFO {
        cbSize: u32::try_from(size_of::<MONITORINFO>()).unwrap_or(0),
        ..MONITORINFO::default()
    };
    // SAFETY: `info` has `cbSize` set and lives for the call.
    if !unsafe { GetMonitorInfoW(hmon, &raw mut info) }.as_bool() {
        // SAFETY: a point by value and a flag that asks for the primary monitor.
        hmon = unsafe { MonitorFromPoint(POINT::default(), MONITOR_DEFAULTTOPRIMARY) };
        // SAFETY: as above.
        let _ = unsafe { GetMonitorInfoW(hmon, &raw mut info) };
    }
    let (mut dpi_x, mut dpi_y) = (96_u32, 96_u32);
    // SAFETY: both out pointers are valid for the call; on failure they keep 96.
    let _ = unsafe { GetDpiForMonitor(hmon, MDT_EFFECTIVE_DPI, &raw mut dpi_x, &raw mut dpi_y) };
    let scale = f64::from(dpi_x) / 96.0;
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: scaled(CLIENT_DIP.0, dpi_x),
        bottom: scaled(CLIENT_DIP.1, dpi_x),
    };
    // SAFETY: `rect` is a valid in/out rectangle.
    let _ = unsafe { AdjustWindowRectExForDpi(&raw mut rect, STYLE, false, EX_STYLE, dpi_x) };
    let (width, height) = (rect.right - rect.left, rect.bottom - rect.top);
    let work = info.rcWork;
    let x = work.left + ((work.right - work.left - width) / 2).max(0);
    let y = work.top + ((work.bottom - work.top - height) / 2).max(0);
    let placed = RECT {
        left: x,
        top: y,
        right: x + width,
        bottom: y + height,
    };
    (placed, scale)
}

fn set_dark_caption(hwnd: HWND) {
    let on: i32 = 1;
    // SAFETY: the attribute takes a 4-byte BOOL; `on` lives for the call. A failure (older
    // Windows) just leaves the light caption.
    let _ = unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            (&raw const on).cast::<c_void>(),
            u32::try_from(size_of::<i32>()).unwrap_or(4),
        )
    };
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn create(state: &Rc<State>, title: &str, monitor: isize) -> Result<HWND, ShellError> {
    // SAFETY: a null name asks for the handle of the running executable; no pointer is read.
    let instance: HINSTANCE = unsafe { GetModuleHandleW(None) }?.into();
    register_class(instance)?;
    let (rect, _) = placement(monitor);
    let title = wide(title);
    let param = Rc::into_raw(Rc::clone(state));
    // SAFETY: the class is registered; `title` outlives the call; `param` is a pointer from
    // `Rc::into_raw` that the window procedure takes over at WM_NCCREATE.
    let created = unsafe {
        CreateWindowExW(
            EX_STYLE,
            CLASS,
            PCWSTR(title.as_ptr()),
            STYLE,
            rect.left,
            rect.top,
            rect.right - rect.left,
            rect.bottom - rect.top,
            None,
            None,
            Some(instance),
            Some(param.cast::<c_void>()),
        )
    };
    if created.is_err() && !state.attached.get() {
        // SAFETY: creation failed before the window took the pointer, so the count is still ours.
        drop(unsafe { Rc::from_raw(param) });
    }
    created
}

fn bring_to_front(hwnd: HWND) {
    // SAFETY: `hwnd` is a live window of this thread; the calls only change its show state.
    unsafe {
        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }
        if !SetForegroundWindow(hwnd).as_bool() {
            // Windows refused to steal the focus: ask for attention on the taskbar instead.
            let info = FLASHWINFO {
                cbSize: u32::try_from(size_of::<FLASHWINFO>()).unwrap_or(0),
                hwnd,
                dwFlags: FLASHW_ALL | FLASHW_TIMERNOFG,
                uCount: 3,
                dwTimeout: 0,
            };
            let _ = FlashWindowEx(&raw const info);
        }
    }
}

impl SheetWindow {
    /// Opens the window centered on `monitor` (an `HMONITOR` value; 0 or an invalid one falls
    /// back to the primary monitor), titled `title`, brings it to the front, and reports through
    /// `on_event` on the UI thread. `on_event` is never called from inside this function: events
    /// raised while the window is created are delivered from the next message. `on_panic` is
    /// called once, with the message, when `on_event` or a window callback panics; the window
    /// then sends no more events (the app decides whether to close it).
    ///
    /// # Errors
    /// Returns the Win32 error when the class or the window cannot be created.
    pub fn open(
        monitor: isize,
        title: &str,
        on_event: Box<dyn FnMut(SheetEvent)>,
        on_panic: Box<dyn Fn(String)>,
    ) -> Result<Self, ShellError> {
        Self::open_with(monitor, title, on_event, on_panic, true)
    }

    fn open_with(
        monitor: isize,
        title: &str,
        on_event: Box<dyn FnMut(SheetEvent)>,
        on_panic: Box<dyn Fn(String)>,
        show: bool,
    ) -> Result<Self, ShellError> {
        let state = Rc::new(State::new(on_event, on_panic));
        let hwnd = create(&state, title, monitor)?;
        let window = Self {
            hwnd: hwnd.0 as isize,
            state,
        };
        set_dark_caption(hwnd);
        clipboard::set_owner(window.hwnd);
        if show {
            // SAFETY: `hwnd` was just created on this thread and is alive.
            let _ = unsafe { ShowWindow(hwnd, SW_SHOW) };
            bring_to_front(hwnd);
        }
        window.state.emit_resized(hwnd);
        window.state.set_ready();
        // SAFETY: posts to our own live window; the flush runs from the message loop.
        let _ = unsafe { PostMessageW(Some(hwnd), WM_APP_FLUSH, WPARAM(0), LPARAM(0)) };
        Ok(window)
    }

    /// The window, as the integer value of its `HWND`.
    #[must_use]
    pub fn hwnd(&self) -> isize {
        self.hwnd
    }

    /// Client width and height in pixels and the scale (pixels per DIP).
    #[must_use]
    pub fn client_px(&self) -> (u32, u32, f64) {
        client_metrics(hwnd_of(self.hwnd))
    }

    /// Restores the window if minimized and brings it to the front; flashes the taskbar button
    /// when Windows refuses the focus.
    pub fn raise(&self) {
        bring_to_front(hwnd_of(self.hwnd));
    }

    /// Sets the caption text.
    pub fn set_title(&self, title: &str) {
        let title = wide(title);
        // SAFETY: `title` is NUL-terminated and lives for the call; the window is ours.
        let _ = unsafe { SetWindowTextW(hwnd_of(self.hwnd), PCWSTR(title.as_ptr())) };
    }

    /// Destroys the window. No [`SheetEvent::Closed`] follows: the caller asked for it.
    pub fn close(self) {
        drop(self);
    }
}

impl Drop for SheetWindow {
    fn drop(&mut self) {
        self.state.mute();
        clipboard::clear_owner(self.hwnd);
        destroy(self.hwnd);
    }
}
