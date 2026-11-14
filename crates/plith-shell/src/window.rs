//! The notch window: class registration, creation, the window procedure and the message loop.
//!
//! One fixed topmost window is placed once at the surface rectangle and never moves. Depending
//! on the hit candidate a second input window sits beside it (see `hit`).

pub(crate) mod state;

use crate::hit::apply::apply_latest;
use crate::poster::{QuitHandle, ShapePoster, WM_APP_SHAPE};
use crate::suppress::{self, WM_APP_BAR};
use crate::topmost;
use plith_core::launch::HitMode;
use plith_core::layout::{RectPx, SurfaceLayout};
use plith_core::scene::{ShapeCell, Suppression};
use state::{Inner, current, set_current};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use windows::Win32::Foundation::{
    COLORREF, ERROR_CLASS_ALREADY_EXISTS, GetLastError, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, HWND_TOPMOST,
    IDC_ARROW, IsWindow, LWA_ALPHA, LoadCursorW, MA_NOACTIVATE, MSG, PostQuitMessage,
    RegisterClassExW, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SetLayeredWindowAttributes, SetWindowPos,
    ShowWindow, TranslateMessage, WINDOW_EX_STYLE, WM_DESTROY, WM_DISPLAYCHANGE, WM_DPICHANGED,
    WM_LBUTTONUP, WM_MOUSEACTIVATE, WM_POWERBROADCAST, WM_TIMER, WM_WTSSESSION_CHANGE, WNDCLASSEXW,
    WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    WS_EX_TRANSPARENT, WS_POPUP,
};
use windows::core::{PCWSTR, w};

const VISUAL_CLASS: PCWSTR = w!("Plith.Notch");
const INPUT_CLASS: PCWSTR = w!("Plith.NotchInput");

/// Something the app has to react to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShellEvent {
    /// The left button was released inside the island shape.
    Click,
    /// The DPI or the display configuration changed; relay the surface when the island is at rest.
    DisplayChanged,
    /// The suppression state changed; republish the scene with it.
    Suppression(Suppression),
}

/// Handle to the notch window(s). It lives on the UI thread and is not `Send`.
pub struct Shell {
    inner: Rc<Inner>,
}

fn hwnd_of(raw: isize) -> HWND {
    HWND(raw as *mut core::ffi::c_void)
}

fn module() -> windows::core::Result<HINSTANCE> {
    // SAFETY: a null name asks for the handle of the running executable.
    Ok(unsafe { GetModuleHandleW(None) }?.into())
}

fn register_class(name: PCWSTR, instance: HINSTANCE) -> windows::core::Result<()> {
    // SAFETY: the stock arrow cursor needs no instance.
    let cursor = unsafe { LoadCursorW(None, IDC_ARROW) }?;
    let class = WNDCLASSEXW {
        cbSize: u32::try_from(size_of::<WNDCLASSEXW>()).unwrap_or(0),
        lpfnWndProc: Some(window_proc),
        hInstance: instance,
        hCursor: cursor,
        lpszClassName: name,
        ..WNDCLASSEXW::default()
    };
    // SAFETY: `class` is fully initialised and `name` is a static string.
    let atom = unsafe { RegisterClassExW(&raw const class) };
    // SAFETY: reads the calling thread's last error.
    if atom == 0 && unsafe { GetLastError() } != ERROR_CLASS_ALREADY_EXISTS {
        return Err(windows::core::Error::from_thread());
    }
    Ok(())
}

fn create_window(
    class: PCWSTR,
    style: WINDOW_EX_STYLE,
    rect: RectPx,
    instance: HINSTANCE,
) -> windows::core::Result<HWND> {
    // SAFETY: the class is registered and the arguments are plain values; the window procedure
    // may run during this call and finds the thread's state through `current()`.
    unsafe {
        CreateWindowExW(
            style,
            class,
            w!(""),
            WS_POPUP,
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            None,
            None,
            Some(instance),
            None,
        )
    }
}

fn place(hwnd: HWND, rect: RectPx) -> windows::core::Result<()> {
    // SAFETY: `hwnd` was just created on this thread. This is the only placement ever made.
    unsafe {
        SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            SWP_NOACTIVATE,
        )
    }
}

fn build(inner: &Inner) -> windows::core::Result<()> {
    let instance = module()?;
    register_class(VISUAL_CLASS, instance)?;
    register_class(INPUT_CLASS, instance)?;
    let rect = inner.layout.surface;
    let base = WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_NOREDIRECTIONBITMAP;
    let visual_style = if inner.hit_mode == HitMode::Split {
        base | WS_EX_TRANSPARENT | WS_EX_LAYERED
    } else {
        base
    };
    let visual = create_window(VISUAL_CLASS, visual_style, rect, instance)?;
    inner.visual.set(visual.0 as isize);
    place(visual, rect)?;
    if inner.hit_mode == HitMode::Split {
        // SAFETY: `visual` is a layered window of this thread.
        unsafe { SetLayeredWindowAttributes(visual, COLORREF(0), 255, LWA_ALPHA) }?;
    }
    let input_style = match inner.hit_mode {
        HitMode::Region => None,
        HitMode::Split => Some(base),
        HitMode::Mask => Some(WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED),
    };
    if let Some(style) = input_style {
        let input = create_window(INPUT_CLASS, style, rect, instance)?;
        inner.input.set(input.0 as isize);
        place(input, rect)?;
    }
    suppress::register(inner, visual)?;
    let hook = topmost::install_hook()?;
    inner.registrations.borrow_mut().win_event_hook = Some(hook);
    apply_latest(inner);
    Ok(())
}

impl Shell {
    /// Creates the hidden notch window(s) for `layout` with the chosen hit candidate. The window
    /// is not shown; call [`Shell::show`]. `on_event` runs on the UI thread only.
    ///
    /// # Errors
    /// Returns the Win32 error of the first call that fails; nothing is left behind.
    pub fn create(
        layout: SurfaceLayout,
        hit: HitMode,
        shapes: Arc<ShapeCell>,
        on_event: Box<dyn FnMut(ShellEvent)>,
    ) -> windows::core::Result<Self> {
        let inner = Rc::new(Inner::new(layout, hit, shapes, on_event));
        set_current(Some(Rc::clone(&inner)));
        let shell = Self { inner };
        // On failure `shell` is dropped here, which destroys whatever was created.
        build(&shell.inner)?;
        Ok(shell)
    }

    /// The visual window, as the integer value of its `HWND`.
    #[must_use]
    pub fn hwnd(&self) -> isize {
        self.inner.visual.get()
    }

    /// Shows every shell window without activating it.
    pub fn show(&self) {
        for hwnd in self.inner.windows() {
            // SAFETY: the handles are windows of this thread.
            let _ = unsafe { ShowWindow(hwnd_of(hwnd), SW_SHOWNOACTIVATE) };
        }
    }

    /// A handle for the render thread to post drawn shapes.
    #[must_use]
    pub fn shape_poster(&self) -> ShapePoster {
        ShapePoster {
            hwnd: self.hwnd(),
            cell: Arc::clone(&self.inner.shapes),
            pending: Arc::clone(&self.inner.pending),
        }
    }

    /// A handle that closes the window from any thread.
    #[must_use]
    pub fn quit_handle(&self) -> QuitHandle {
        QuitHandle { hwnd: self.hwnd() }
    }

    /// Hit-test update cost on the UI thread: `(count, mean_ms, max_ms)`.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn hit_stats(&self) -> (u64, f64, f64) {
        let stats = self.inner.stats.get();
        let mean = if stats.count == 0 {
            0.0
        } else {
            stats.total_ms / stats.count as f64
        };
        (stats.count, mean, stats.max_ms)
    }

    /// How many hit-test updates failed.
    #[must_use]
    pub fn hit_errors(&self) -> u64 {
        self.inner.hit_errors.get()
    }

    /// Class name of the window that sat above ours at the last foreground change, if any.
    #[must_use]
    pub fn last_culprit(&self) -> Option<String> {
        self.inner.culprit.borrow().clone()
    }

    /// The current suppression state.
    #[must_use]
    pub fn suppression(&self) -> Suppression {
        self.inner.suppression.get()
    }
}

impl Drop for Shell {
    fn drop(&mut self) {
        let visual = hwnd_of(self.inner.visual.get());
        // SAFETY: IsWindow accepts any handle; DestroyWindow runs on the owning thread because
        // `Shell` is not `Send`. WM_DESTROY still finds the state, which is cleared afterwards.
        unsafe {
            if IsWindow(Some(visual)).as_bool() {
                let _ = DestroyWindow(visual);
            }
        }
        set_current(None);
    }
}

/// Runs the message loop until `WM_QUIT` and returns its exit code (negative on a loop error).
#[must_use]
pub fn run_message_loop() -> i32 {
    let mut msg = MSG::default();
    loop {
        // SAFETY: `msg` is a valid MSG for the call.
        let got = unsafe { GetMessageW(&raw mut msg, None, 0, 0) };
        match got.0 {
            0 => return i32::try_from(msg.wParam.0).unwrap_or(0),
            -1 => return -1,
            _ => {
                // SAFETY: `msg` was filled by GetMessageW.
                unsafe {
                    let _ = TranslateMessage(&raw const msg);
                    DispatchMessageW(&raw const msg);
                }
            }
        }
    }
}

fn on_click(inner: &Inner, lparam: LPARAM) {
    let Some(frame) = inner.shapes.latest() else {
        return;
    };
    #[allow(clippy::cast_possible_truncation)]
    let (x, y) = (
        f64::from(lparam.0 as i16),
        f64::from((lparam.0 >> 16) as i16),
    );
    let scale = inner.layout.scale;
    let (ox, oy) = inner.layout.island_origin(frame.shape);
    if frame.shape.contains(x / scale - ox, y / scale - oy) {
        inner.emit(ShellEvent::Click);
    }
}

fn on_destroy(inner: &Inner, hwnd: HWND) {
    topmost::kill_timer(inner);
    if let Some(hook) = inner.registrations.borrow_mut().win_event_hook.take() {
        topmost::remove_hook(hook);
    }
    suppress::unregister(inner, hwnd);
    inner.mask.borrow_mut().take();
    let input = inner.input.replace(0);
    if input != 0 {
        // SAFETY: the input window belongs to this thread.
        let _ = unsafe { DestroyWindow(hwnd_of(input)) };
    }
    // SAFETY: posts WM_QUIT to this thread's queue.
    unsafe { PostQuitMessage(0) };
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let Some(inner) = current() else {
        // SAFETY: forwards the unhandled message unchanged.
        return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
    };
    let is_visual = hwnd.0 as isize == inner.visual.get();
    match msg {
        WM_MOUSEACTIVATE => return LRESULT(isize::try_from(MA_NOACTIVATE).unwrap_or(0)),
        WM_LBUTTONUP => {
            on_click(&inner, lparam);
            return LRESULT(0);
        }
        WM_DPICHANGED | WM_DISPLAYCHANGE => {
            inner.emit(ShellEvent::DisplayChanged);
            return LRESULT(0);
        }
        WM_APP_SHAPE if is_visual => {
            inner.pending.store(false, Ordering::Release);
            apply_latest(&inner);
            return LRESULT(0);
        }
        WM_TIMER if is_visual && wparam.0 == topmost::RETRY_TIMER_ID => {
            topmost::on_timer(&inner);
            return LRESULT(0);
        }
        WM_POWERBROADCAST if is_visual => suppress::on_power(&inner, wparam, lparam),
        WM_WTSSESSION_CHANGE if is_visual => suppress::on_session(&inner, wparam),
        WM_APP_BAR if is_visual => suppress::on_appbar(&inner, wparam, lparam),
        WM_DESTROY if is_visual => {
            on_destroy(&inner, hwnd);
            return LRESULT(0);
        }
        _ => {}
    }
    // SAFETY: forwards the unhandled message unchanged.
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}
