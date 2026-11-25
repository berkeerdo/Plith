//! The installer window: a borderless 640 x 420 DIP popup with rounded corners and its own class.
//!
//! It draws nothing. The render thread presents to its `HWND` (`plith_render::sheet`); this module
//! turns Win32 input into [`SetupEvent`]s in DIP, drags the window by the areas the current
//! screen names, and keeps its size and DPI honest. It is the model of [`crate::sheet_window`],
//! without a caption: the screen draws its own close mark.

mod proc;
#[cfg(test)]
mod tests;

use crate::handles::hwnd_of;
use plith_core::setup::{SETUP_H, SETUP_W, SetupTarget};
use proc::{State, WM_APP_FLUSH, client_metrics, destroy, window_proc};
use std::ffi::c_void;
use std::rc::Rc;
use windows::Win32::Foundation::{
    ERROR_CLASS_ALREADY_EXISTS, GetLastError, HINSTANCE, HWND, LPARAM, POINT, RECT, WPARAM,
};
use windows::Win32::Graphics::Dwm::{
    DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmSetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITOR_DEFAULTTOPRIMARY, MONITORINFO,
    MonitorFromPoint,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, FindWindowW, GetCursorPos, GetSystemMetrics, HICON, IDC_ARROW, IMAGE_ICON,
    IsIconic, LR_SHARED, LoadCursorW, LoadImageW, PostMessageW, PostQuitMessage, RegisterClassExW,
    SM_CXICON, SM_CXSMICON, SW_RESTORE, SW_SHOW, SYSTEM_METRICS_INDEX, SetForegroundWindow,
    ShowWindow, WINDOW_EX_STYLE, WINDOW_STYLE, WM_APP, WNDCLASSEXW, WS_EX_NOREDIRECTIONBITMAP,
    WS_POPUP,
};
use windows::core::{PCWSTR, w};

/// Why the installer window could not be created.
pub type ShellError = windows::core::Error;

const CLASS: PCWSTR = w!("Plith.Setup");
/// The resource id the build script gives the exe's icon.
const ICON_RESOURCE: usize = 1;
const STYLE: WINDOW_STYLE = WS_POPUP;
const EX_STYLE: WINDOW_EX_STYLE = WS_EX_NOREDIRECTIONBITMAP;

/// Posted to the window from any thread to say "look at your queue": it becomes
/// [`SetupEvent::Wake`] on the UI thread.
pub const WM_SETUP_WAKE: u32 = WM_APP + 2;

/// Something that happened in the installer window. Coordinates are DIP relative to the client
/// area's top-left.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SetupEvent {
    /// The left button was pressed and released at `(x, y)`.
    Click(f64, f64),
    /// A key went down (a `VK_*` code); auto-repeat is dropped.
    Key(u32),
    /// The window has a new scale (pixels per DIP): shown first, and after a monitor change.
    Dpi(f64),
    /// [`WM_SETUP_WAKE`] arrived.
    Wake,
    /// The user asked to close the window (Alt+F4). It stays alive until the owner drops the
    /// handle.
    Closed,
}

/// Handle to the installer window. It lives on the UI thread and is not `Send`. Dropping it
/// destroys the window without sending [`SetupEvent::Closed`].
pub struct SetupWindow {
    hwnd: isize,
    state: Rc<State>,
}

/// The exe's icon (resource 1) at the system size `metric`; a null icon when it carries none.
fn class_icon(instance: HINSTANCE, metric: SYSTEM_METRICS_INDEX) -> HICON {
    // SAFETY: the metric call has no pointer input; the resource id is the integer form of a
    // name in this module; LR_SHARED images are owned by the system and never destroyed.
    unsafe {
        let size = GetSystemMetrics(metric);
        LoadImageW(
            Some(instance),
            PCWSTR(ICON_RESOURCE as *const u16),
            IMAGE_ICON,
            size,
            size,
            LR_SHARED,
        )
        .map(|handle| HICON(handle.0))
        .unwrap_or_default()
    }
}

fn register_class(instance: HINSTANCE) -> windows::core::Result<()> {
    // SAFETY: the stock arrow cursor is a system resource, so a null instance and a predefined id are valid.
    let cursor = unsafe { LoadCursorW(None, IDC_ARROW) }?;
    let class = WNDCLASSEXW {
        cbSize: u32::try_from(size_of::<WNDCLASSEXW>()).unwrap_or(0),
        lpfnWndProc: Some(window_proc),
        hInstance: instance,
        hCursor: cursor,
        hIcon: class_icon(instance, SM_CXICON),
        hIconSm: class_icon(instance, SM_CXSMICON),
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
fn scaled(dip: f64, dpi: u32) -> i32 {
    // The window is a few hundred pixels: the cast cannot truncate.
    #[allow(clippy::cast_possible_truncation)]
    let px = (dip * f64::from(dpi) / 96.0).round() as i32;
    px
}

/// The window rectangle in pixels: the whole window at the scale of the monitor under the mouse
/// pointer, centred on its work area. The pointer is only read, never moved.
fn placement() -> RECT {
    let mut cursor = POINT::default();
    // SAFETY: `cursor` is a valid out pointer; on failure it stays at the origin, which names a
    // monitor all the same.
    let _ = unsafe { GetCursorPos(&raw mut cursor) };
    // SAFETY: a point by value and a flag that asks for the nearest monitor.
    let mut hmon = unsafe { MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST) };
    let mut info = MONITORINFO {
        cbSize: u32::try_from(size_of::<MONITORINFO>()).unwrap_or(0),
        ..MONITORINFO::default()
    };
    // SAFETY: `info` has `cbSize` set and lives for the call.
    if !unsafe { GetMonitorInfoW(hmon, &raw mut info) }.as_bool() {
        // SAFETY: as above, asking for the primary monitor.
        hmon = unsafe { MonitorFromPoint(POINT::default(), MONITOR_DEFAULTTOPRIMARY) };
        // SAFETY: as above.
        let _ = unsafe { GetMonitorInfoW(hmon, &raw mut info) };
    }
    let (mut dpi_x, mut dpi_y) = (96_u32, 96_u32);
    // SAFETY: both out pointers are valid for the call; on failure they keep 96.
    let _ = unsafe { GetDpiForMonitor(hmon, MDT_EFFECTIVE_DPI, &raw mut dpi_x, &raw mut dpi_y) };
    let (width, height) = (scaled(SETUP_W, dpi_x), scaled(SETUP_H, dpi_x));
    let work = info.rcWork;
    let x = work.left + ((work.right - work.left - width) / 2).max(0);
    let y = work.top + ((work.bottom - work.top - height) / 2).max(0);
    RECT {
        left: x,
        top: y,
        right: x + width,
        bottom: y + height,
    }
}

fn round_corners(hwnd: HWND) {
    let preference = DWMWCP_ROUND;
    // SAFETY: the attribute takes a 4-byte enum value that lives for the call. A failure (older
    // Windows) just leaves square corners.
    let _ = unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            (&raw const preference).cast::<c_void>(),
            u32::try_from(size_of_val(&preference)).unwrap_or(4),
        )
    };
}

fn create(state: &Rc<State>, title: &str) -> Result<HWND, ShellError> {
    // SAFETY: a null name asks for the handle of the running executable; no pointer is read.
    let instance: HINSTANCE = unsafe { GetModuleHandleW(None) }?.into();
    register_class(instance)?;
    let rect = placement();
    let title: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
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

/// Restores the window when minimized and brings it to the front. Windows may refuse the focus.
fn bring_to_front(hwnd: HWND) {
    // SAFETY: `hwnd` is a window handle; the calls only change its show state and ignore a
    // handle that is not a window.
    unsafe {
        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }
        let _ = SetForegroundWindow(hwnd);
    }
}

/// The installer window of another process of this user, as the integer value of its `HWND`.
#[must_use]
pub fn find_open() -> Option<isize> {
    // SAFETY: the class name is a static NUL-terminated string; no title filter.
    unsafe { FindWindowW(CLASS, PCWSTR::null()) }
        .ok()
        .map(|hwnd| hwnd.0 as isize)
}

/// Restores and focuses the window `hwnd` (from [`find_open`]), which may belong to another
/// process.
pub fn raise_other(hwnd: isize) {
    bring_to_front(hwnd_of(hwnd));
}

/// Posts [`WM_SETUP_WAKE`] to the window `hwnd` from any thread; a dead window ignores it.
pub fn wake(hwnd: isize) {
    // SAFETY: posting to a handle value that is not (or no longer) a window just fails.
    let _ = unsafe { PostMessageW(Some(hwnd_of(hwnd)), WM_SETUP_WAKE, WPARAM(0), LPARAM(0)) };
}

/// Ends the message loop of the calling thread.
pub fn quit() {
    // SAFETY: a plain call that posts `WM_QUIT` to this thread's queue.
    unsafe { PostQuitMessage(0) };
}

impl SetupWindow {
    /// Opens the window on the monitor under the pointer, titled `title`, brings it to the
    /// front, and reports through `on_event` on the UI thread. `on_event` is never called from
    /// inside this function: events raised while the window is created are delivered from the
    /// next message. `on_panic` is called once, with the message, when `on_event` or a window
    /// callback panics; the window then sends no more events.
    ///
    /// # Errors
    /// Returns the Win32 error when the class or the window cannot be created.
    pub fn open(
        title: &str,
        on_event: Box<dyn FnMut(SetupEvent)>,
        on_panic: Box<dyn Fn(String)>,
    ) -> Result<Self, ShellError> {
        Self::open_with(title, on_event, on_panic, true)
    }

    /// `show` false leaves the window hidden and unactivated, for tests.
    fn open_with(
        title: &str,
        on_event: Box<dyn FnMut(SetupEvent)>,
        on_panic: Box<dyn Fn(String)>,
        show: bool,
    ) -> Result<Self, ShellError> {
        let state = Rc::new(State::new(on_event, on_panic));
        let hwnd = create(&state, title)?;
        let window = Self {
            hwnd: hwnd.0 as isize,
            state,
        };
        round_corners(hwnd);
        if show {
            // SAFETY: `hwnd` was just created on this thread and is alive.
            let _ = unsafe { ShowWindow(hwnd, SW_SHOW) };
            bring_to_front(hwnd);
        }
        window.state.emit_dpi(hwnd);
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

    /// The click targets of the screen on show: the areas whose action is
    /// [`SetupAction::Drag`](plith_core::setup::SetupAction::Drag) drag the window.
    pub fn set_targets(&self, targets: Vec<SetupTarget>) {
        *self.state.targets.borrow_mut() = targets;
    }

    /// Destroys the window. No [`SetupEvent::Closed`] follows: the caller asked for it.
    pub fn close(self) {
        drop(self);
    }
}

impl Drop for SetupWindow {
    fn drop(&mut self) {
        self.state.mute();
        destroy(self.hwnd);
    }
}
