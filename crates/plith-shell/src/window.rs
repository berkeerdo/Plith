//! The notch window: class registration, creation, the window procedure and the message loop.
//!
//! One topmost window is placed at the surface rectangle and moved only by a display change
//! (`display`). Its window region follows the island shape, so clicks outside the shape fall
//! through (see `hit`).

pub(crate) mod display;
mod event;
pub(crate) mod mouse;
pub(crate) mod state;

pub use event::ShellEvent;

use crate::handles::hwnd_of;
use crate::hit::apply::apply_latest;
use crate::poster::{QuitHandle, ShapePoster, UiWaker, WM_APP_BAR, WM_APP_SHAPE, WM_APP_WAKE};
use crate::suppress;
use crate::timer::{TimerHandle, is_app_timer};
use crate::topmost;
use plith_core::display::Display;
use plith_core::layout::RectPx;
use plith_core::panic_text::payload_text;
use plith_core::scene::{ShapeCell, Suppression};
use state::{Inner, current, set_current};
use std::marker::PhantomData;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;
use std::sync::Arc;
use windows::Win32::Foundation::{
    ERROR_CLASS_ALREADY_EXISTS, GetLastError, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, HWND_TOPMOST,
    IDC_ARROW, IsWindow, LoadCursorW, MA_NOACTIVATE, MSG, PostQuitMessage, RegisterClassExW,
    SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SetWindowPos, ShowWindow, TranslateMessage, WINDOW_EX_STYLE,
    WM_CAPTURECHANGED, WM_CLOSE, WM_DESTROY, WM_DISPLAYCHANGE, WM_DPICHANGED, WM_LBUTTONDOWN,
    WM_LBUTTONUP, WM_MOUSEACTIVATE, WM_MOUSEMOVE, WM_POWERBROADCAST, WM_TIMER,
    WM_WTSSESSION_CHANGE, WNDCLASSEXW, WS_EX_NOACTIVATE, WS_EX_NOREDIRECTIONBITMAP,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};
use windows::core::{PCWSTR, w};

const VISUAL_CLASS: PCWSTR = w!("Plith.Notch");

/// Runs `f` and turns a panic into `Err("<label> panicked: <payload>")`. The window procedure
/// and the foreground hook run every callback through it: a panic must not unwind out of an
/// `extern "system"` function.
///
/// # Errors
/// Returns the formatted panic message when `f` panics.
pub fn guard(label: &str, f: impl FnOnce()) -> Result<(), String> {
    catch_unwind(AssertUnwindSafe(f))
        .map_err(|payload| format!("{label} panicked: {}", payload_text(&*payload)))
}

/// Handle to the notch window. It lives on the UI thread and is not `Send`.
pub struct Shell {
    pub(crate) inner: Rc<Inner>,
}

fn module() -> windows::core::Result<HINSTANCE> {
    // SAFETY: a null name asks for the handle of the running executable; no pointer is read.
    Ok(unsafe { GetModuleHandleW(None) }?.into())
}

fn register_class(name: PCWSTR, instance: HINSTANCE) -> windows::core::Result<()> {
    // SAFETY: the stock arrow cursor is a system resource, so a null instance and a predefined id are valid.
    let cursor = unsafe { LoadCursorW(None, IDC_ARROW) }?;
    let class = WNDCLASSEXW {
        cbSize: u32::try_from(size_of::<WNDCLASSEXW>()).unwrap_or(0),
        lpfnWndProc: Some(window_proc),
        hInstance: instance,
        hCursor: cursor,
        lpszClassName: name,
        ..WNDCLASSEXW::default()
    };
    // SAFETY: `class` is fully initialised (cbSize set) and `name` is a static NUL-terminated string that outlives the class.
    let atom = unsafe { RegisterClassExW(&raw const class) };
    // SAFETY: GetLastError has no preconditions; RegisterClassExW just failed on this thread.
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
    // SAFETY: `hwnd` was just created on this thread and is still alive; the call only repositions it.
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
    let rect = inner.layout().surface;
    let style = WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_NOREDIRECTIONBITMAP;
    let visual = create_window(VISUAL_CLASS, style, rect, instance)?;
    inner.visual.set(visual.0 as isize);
    place(visual, rect)?;
    suppress::register(inner, visual)?;
    if let Err(error) = crate::drag_in::register(visual) {
        *inner.drop_error.borrow_mut() = Some(error.to_string());
    }
    let hook = topmost::install_hook()?;
    inner.registrations.borrow_mut().win_event_hook = Some(hook);
    apply_latest(inner);
    Ok(())
}

impl Shell {
    /// Creates the hidden notch window for `display`. The window is not shown; call
    /// [`Shell::show`]. A later display change moves it (see [`ShellEvent::DisplayChanged`]).
    /// `on_event` runs on the UI thread only. `on_panic` is called once, on the UI thread, with
    /// the message of a panic caught in a window callback; the shell then closes the window and
    /// never enters `on_event` again.
    ///
    /// # Errors
    /// Returns the Win32 error of the first call that fails; nothing is left behind.
    pub fn create(
        display: Display,
        shapes: Arc<ShapeCell>,
        on_event: Box<dyn FnMut(ShellEvent)>,
        on_panic: Box<dyn Fn(String)>,
    ) -> windows::core::Result<Self> {
        let inner = Rc::new(Inner::new(display, shapes, on_event, on_panic));
        set_current(Some(Rc::clone(&inner)));
        let shell = Self { inner };
        // On failure `shell` is dropped here, which destroys whatever was created.
        build(&shell.inner)?;
        Ok(shell)
    }

    /// The notch window, as the integer value of its `HWND`.
    #[must_use]
    pub fn hwnd(&self) -> isize {
        self.inner.visual.get()
    }

    /// Shows the shell window without activating it.
    pub fn show(&self) {
        for hwnd in self.inner.windows() {
            // SAFETY: the handles are live windows of this thread, so ShowWindow's window argument is valid.
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

    /// A handle for service threads to wake the UI thread; see [`UiWaker`].
    #[must_use]
    pub fn ui_waker(&self) -> UiWaker {
        UiWaker {
            hwnd: self.hwnd(),
            pending: Arc::clone(&self.inner.wake),
        }
    }

    /// A handle that sets and kills app timers on this window, from the UI thread.
    #[must_use]
    pub fn timer_handle(&self) -> TimerHandle {
        TimerHandle {
            hwnd: self.hwnd(),
            _ui_thread: PhantomData,
        }
    }

    /// Starts or restarts app timer `id` with a period of `ms`; see [`TimerHandle::set`].
    pub fn set_timer(&self, id: usize, ms: u32) {
        self.timer_handle().set(id, ms);
    }

    /// Stops app timer `id`; see [`TimerHandle::kill`].
    pub fn kill_timer(&self, id: usize) {
        self.timer_handle().kill(id);
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

    /// Why the window takes no file drops, when the drop target could not be registered.
    #[must_use]
    pub fn drop_target_error(&self) -> Option<String> {
        self.inner.drop_error.borrow().clone()
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
        crate::drag_in::revoke(visual);
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
        // SAFETY: `msg` is a valid, writable MSG for the whole call.
        let got = unsafe { GetMessageW(&raw mut msg, None, 0, 0) };
        match got.0 {
            0 => return i32::try_from(msg.wParam.0).unwrap_or(0),
            -1 => return -1,
            _ => {
                // SAFETY: `msg` was filled by GetMessageW just above and is not modified before dispatch.
                unsafe {
                    let _ = TranslateMessage(&raw const msg);
                    DispatchMessageW(&raw const msg);
                }
            }
        }
    }
}

fn on_destroy(inner: &Inner, hwnd: HWND) {
    crate::tray::remove(inner, hwnd);
    crate::drag_in::revoke(hwnd);
    topmost::kill_timer(inner);
    if let Some(hook) = inner.registrations.borrow_mut().win_event_hook.take() {
        topmost::remove_hook(hook);
    }
    suppress::unregister(inner, hwnd);
    // Runs only from WM_DESTROY, so a window was created and is being destroyed: a failed
    // `CreateWindowExW` never reaches this call.
    // SAFETY: posts WM_QUIT to this thread's queue.
    unsafe { PostQuitMessage(0) };
}

/// Calls the default window procedure.
fn default_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // SAFETY: the four values are exactly what the system passed to our window procedure for
    // this window, which is what DefWindowProcW requires.
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let Some(inner) = current() else {
        return default_proc(hwnd, msg, wparam, lparam);
    };
    let mut handled = None;
    let outcome = guard("window procedure", || {
        handled = dispatch(&inner, hwnd, msg, wparam, lparam);
    });
    if let Err(message) = outcome {
        inner.fail(message);
        if msg == WM_DESTROY {
            // The panic skipped `on_destroy`, so nothing else would end the message loop.
            // SAFETY: posts WM_QUIT to this thread's queue.
            unsafe { PostQuitMessage(1) };
        }
    }
    // After a panic the message still gets its default handling (for WM_CLOSE: destruction).
    handled.unwrap_or_else(|| default_proc(hwnd, msg, wparam, lparam))
}

/// Handles one message; `None` means the default procedure should see it too.
fn dispatch(
    inner: &Inner,
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> Option<LRESULT> {
    let is_visual = hwnd.0 as isize == inner.visual.get();
    if is_visual && crate::tray::is_taskbar_created(inner, msg) {
        crate::tray::on_taskbar_created(inner, hwnd);
        return Some(LRESULT(0));
    }
    match msg {
        crate::tray::WM_APP_TRAY if is_visual => {
            crate::tray::on_callback(inner, hwnd, wparam, lparam);
            return Some(LRESULT(0));
        }
        WM_MOUSEACTIVATE => return Some(LRESULT(isize::try_from(MA_NOACTIVATE).unwrap_or(0))),
        // The press only records and captures: `WM_MOUSEACTIVATE` above already answered
        // `MA_NOACTIVATE`, and neither the press nor the capture activates the island.
        WM_LBUTTONDOWN => {
            mouse::on_button_down(inner, lparam);
            return Some(LRESULT(0));
        }
        WM_LBUTTONUP => {
            mouse::on_button_up(inner, lparam);
            return Some(LRESULT(0));
        }
        WM_CAPTURECHANGED => {
            mouse::note_capture_lost(inner);
            return Some(LRESULT(0));
        }
        WM_MOUSEMOVE if is_visual => {
            mouse::on_move(inner, lparam);
            return Some(LRESULT(0));
        }
        mouse::WM_MOUSELEAVE if is_visual => {
            mouse::note_leave(inner);
            return Some(LRESULT(0));
        }
        WM_DPICHANGED | WM_DISPLAYCHANGE if is_visual => {
            display::on_message(inner, hwnd);
            return Some(LRESULT(0));
        }
        display::WM_APP_RELAYOUT if is_visual => {
            display::on_relayout(inner);
            return Some(LRESULT(0));
        }
        WM_APP_SHAPE if is_visual => {
            inner.pending.clear();
            apply_latest(inner);
            return Some(LRESULT(0));
        }
        WM_APP_WAKE if is_visual => {
            inner.wake.clear();
            inner.emit(ShellEvent::Wake);
            return Some(LRESULT(0));
        }
        WM_TIMER if is_visual && wparam.0 == topmost::RETRY_TIMER_ID => {
            topmost::on_timer(inner);
            return Some(LRESULT(0));
        }
        WM_TIMER if is_visual && is_app_timer(wparam.0) => {
            inner.emit(ShellEvent::Timer(wparam.0));
            return Some(LRESULT(0));
        }
        WM_POWERBROADCAST if is_visual => suppress::on_power(inner, wparam, lparam),
        WM_WTSSESSION_CHANGE if is_visual => suppress::on_session(inner, wparam),
        WM_APP_BAR if is_visual => suppress::on_appbar(inner, wparam, lparam),
        // Inside the drag out's modal loop the app handler is mid-call and the window must
        // outlive `DoDragDrop`: the close waits until the drag ends.
        WM_CLOSE if is_visual && inner.dragging_out.get() => {
            inner.close_pending.set(true);
            return Some(LRESULT(0));
        }
        WM_CLOSE if is_visual => inner.emit(ShellEvent::Closing),
        WM_DESTROY if is_visual => {
            on_destroy(inner, hwnd);
            return Some(LRESULT(0));
        }
        _ => {}
    }
    None
}

#[cfg(test)]
mod tests;
