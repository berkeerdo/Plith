//! The settings window's state and window procedure. Separate from the island's procedure: it has
//! its own per-window state behind `GWLP_USERDATA`.

use super::SheetEvent;
use super::clipboard::{clear_owner, read_text};
use super::keys::{CharAssembler, KeyMap, map_char, map_keydown};
use crate::handles::hwnd_of;
use crate::window::guard;
use crate::window::mouse::WM_MOUSELEAVE;
use plith_core::sheet::SheetKey;
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::ValidateRect;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, ReleaseCapture, SetCapture, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
    VK_CONTROL, VK_MENU, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CREATESTRUCTW, DefWindowProcW, DestroyWindow, GWLP_USERDATA, GetClientRect, GetWindowLongPtrW,
    IsWindow, SIZE_MINIMIZED, SWP_NOACTIVATE, SWP_NOZORDER, SetWindowLongPtrW, SetWindowPos,
    WM_CAPTURECHANGED, WM_CHAR, WM_CLOSE, WM_DPICHANGED, WM_ERASEBKGND, WM_KEYDOWN, WM_LBUTTONDOWN,
    WM_LBUTTONUP, WM_MOUSEMOVE, WM_NCCREATE, WM_NCDESTROY, WM_PAINT, WM_SIZE,
};

/// Posted by `open` so events queued while the window was being created are delivered from a
/// later message, never from inside `open`.
pub(super) const WM_APP_FLUSH: u32 = 0x8001;

/// Per-window state, shared by the handle and the window procedure.
pub(super) struct State {
    on_event: RefCell<Box<dyn FnMut(SheetEvent)>>,
    queue: RefCell<VecDeque<SheetEvent>>,
    /// False while `open` runs: events are queued, not delivered.
    ready: Cell<bool>,
    dispatching: Cell<bool>,
    on_panic: Box<dyn Fn(String)>,
    /// True once the callback panicked or the handle was dropped: no further events.
    muted: Cell<bool>,
    /// True once a panic was reported; later panics are covered by the first report.
    panicked: Cell<bool>,
    /// True while a left press that began in this window is held (the window has capture).
    pressed: Cell<bool>,
    /// Pixels per DIP as of the last `Resized`, so mouse moves need no query.
    scale: Cell<f64>,
    /// True once `WM_NCCREATE` stored the pointer that the window owns.
    pub(super) attached: Cell<bool>,
    tracking: Cell<bool>,
    last_size: Cell<Option<(u32, u32, u32)>>,
    chars: RefCell<CharAssembler>,
}

impl State {
    pub(super) fn new(on_event: Box<dyn FnMut(SheetEvent)>, on_panic: Box<dyn Fn(String)>) -> Self {
        Self {
            on_event: RefCell::new(on_event),
            on_panic,
            panicked: Cell::new(false),
            pressed: Cell::new(false),
            scale: Cell::new(1.0),
            queue: RefCell::new(VecDeque::new()),
            ready: Cell::new(false),
            dispatching: Cell::new(false),
            muted: Cell::new(false),
            attached: Cell::new(false),
            tracking: Cell::new(false),
            last_size: Cell::new(None),
            chars: RefCell::new(CharAssembler::default()),
        }
    }

    /// Queues `event`; delivers it at once unless `open` is still running.
    pub(super) fn emit(&self, event: SheetEvent) {
        self.queue.borrow_mut().push_back(event);
        if self.ready.get() {
            self.flush();
        }
    }

    /// Delivers every queued event, in order. Events raised by the callback itself join the queue
    /// and are delivered by this same loop, so the callback is never entered twice at once.
    pub(super) fn flush(&self) {
        if self.dispatching.replace(true) {
            return;
        }
        loop {
            let next = self.queue.borrow_mut().pop_front();
            let Some(event) = next else { break };
            if self.muted.get() {
                continue;
            }
            let mut callback = self.on_event.borrow_mut();
            if let Err(message) = guard("settings window callback", || callback(event)) {
                drop(callback);
                self.fail(message);
            }
        }
        self.dispatching.set(false);
    }

    /// Records a caught panic: reports it once through `on_panic` and mutes the handler. Later
    /// panics are covered by the first report.
    pub(super) fn fail(&self, message: String) {
        self.muted.set(true);
        if self.panicked.replace(true) {
            return;
        }
        // A panic inside the report itself has nowhere left to go.
        let _ = guard("settings window panic report", || (self.on_panic)(message));
    }

    pub(super) fn set_ready(&self) {
        self.ready.set(true);
    }

    pub(super) fn mute(&self) {
        self.muted.set(true);
    }
}

/// Client size in pixels and the scale (pixels per DIP) of `hwnd`.
pub(super) fn client_metrics(hwnd: HWND) -> (u32, u32, f64) {
    let mut rect = RECT::default();
    // SAFETY: `rect` is a valid out pointer; a dead window just leaves it zeroed.
    let _ = unsafe { GetClientRect(hwnd, &raw mut rect) };
    // SAFETY: a plain query on a window handle; it returns 0 for a dead window.
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    let dpi = if dpi == 0 { 96 } else { dpi };
    (
        u32::try_from(rect.right - rect.left).unwrap_or(0),
        u32::try_from(rect.bottom - rect.top).unwrap_or(0),
        f64::from(dpi) / 96.0,
    )
}

impl State {
    pub(super) fn emit_resized(&self, hwnd: HWND) {
        let (width_px, height_px, scale) = client_metrics(hwnd);
        // SAFETY: as in `client_metrics`.
        let dpi = unsafe { GetDpiForWindow(hwnd) };
        self.scale.set(scale);
        let key = (width_px, height_px, dpi);
        if self.last_size.replace(Some(key)) == Some(key) {
            return;
        }
        self.emit(SheetEvent::Resized {
            width_px,
            height_px,
            scale,
        });
    }
}

fn pressed(vk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY) -> bool {
    // SAFETY: a plain query of the calling thread's keyboard state.
    let state = unsafe { GetKeyState(i32::from(vk.0)) };
    state < 0
}

fn signed_word(value: isize) -> i16 {
    let word = u16::try_from(value & 0xFFFF).unwrap_or(0);
    i16::from_ne_bytes(word.to_ne_bytes())
}

fn point(state: &State, lparam: LPARAM) -> (f64, f64) {
    // The low and high words are signed 16-bit client coordinates.
    let x = f64::from(signed_word(lparam.0));
    let y = f64::from(signed_word(lparam.0 >> 16));
    let scale = state.scale.get();
    (x / scale, y / scale)
}

fn on_mouse_move(state: &State, hwnd: HWND, lparam: LPARAM) {
    if !state.tracking.replace(true) {
        let mut track = TRACKMOUSEEVENT {
            cbSize: u32::try_from(size_of::<TRACKMOUSEEVENT>()).unwrap_or(0),
            dwFlags: TME_LEAVE,
            hwndTrack: hwnd,
            dwHoverTime: 0,
        };
        // SAFETY: `track` is fully initialised and lives for the call.
        if unsafe { TrackMouseEvent(&raw mut track) }.is_err() {
            state.tracking.set(false);
        }
    }
    let (x, y) = point(state, lparam);
    state.emit(SheetEvent::Move(x, y));
}

/// A click is a release inside the client area of a press that began in this window.
fn on_button_up(state: &State, hwnd: HWND, lparam: LPARAM) {
    let began_here = state.pressed.replace(false);
    // SAFETY: releases the capture taken at the press; harmless when there is none.
    let _ = unsafe { ReleaseCapture() };
    if !began_here {
        return;
    }
    let (width, height, scale) = client_metrics(hwnd);
    let (x, y) = (signed_word(lparam.0), signed_word(lparam.0 >> 16));
    let inside = x >= 0
        && y >= 0
        && u32::try_from(x).is_ok_and(|x| x < width)
        && u32::try_from(y).is_ok_and(|y| y < height);
    if inside {
        state.emit(SheetEvent::Click(
            f64::from(x) / scale,
            f64::from(y) / scale,
        ));
    }
}

fn on_key_down(state: &State, hwnd: HWND, wparam: WPARAM, lparam: LPARAM) {
    let vk = u16::try_from(wparam.0).unwrap_or(0);
    let mapped = map_keydown(vk, pressed(VK_CONTROL), pressed(VK_SHIFT), pressed(VK_MENU));
    // Bit 30 of `lparam` is set for auto-repeat: a held Ctrl+V pastes once.
    let repeat = lparam.0 & (1 << 30) != 0;
    match mapped {
        Some(KeyMap::Key(key)) => state.emit(SheetEvent::Key(key)),
        Some(KeyMap::Paste) if repeat => {}
        Some(KeyMap::Paste) => {
            if let Some(text) = read_text(hwnd) {
                state.emit(SheetEvent::Key(SheetKey::Paste(text)));
            }
        }
        None => {}
    }
}

fn on_char(state: &State, wparam: WPARAM) {
    let unit = u16::try_from(wparam.0).unwrap_or(0);
    let joined = state.chars.borrow_mut().push(unit);
    if let Some(key) = joined.and_then(map_char) {
        state.emit(SheetEvent::Key(key));
    }
}

fn on_dpi_changed(state: &State, hwnd: HWND, lparam: LPARAM) {
    // SAFETY: for WM_DPICHANGED `lparam` points at the suggested RECT, valid during the message.
    let rect = unsafe { *(lparam.0 as *const RECT) };
    // SAFETY: `hwnd` is the window being processed; only its size and position change.
    let _ = unsafe {
        SetWindowPos(
            hwnd,
            None,
            rect.left,
            rect.top,
            rect.right - rect.left,
            rect.bottom - rect.top,
            SWP_NOZORDER | SWP_NOACTIVATE,
        )
    };
    state.emit_resized(hwnd);
}

/// The close request (caption button, Alt+F4) is only reported: the owner stops the render
/// thread that presents to this window, then destroys it with [`SheetWindow::close`]. A muted
/// window (its handler panicked) reports nothing; its owner closes it after the panic report.
///
/// [`SheetWindow::close`]: super::SheetWindow::close
fn on_close(state: &State) {
    state.emit(SheetEvent::Closed);
}

fn handle(state: &State, hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
    match msg {
        WM_SIZE if wparam.0 != SIZE_MINIMIZED as usize => state.emit_resized(hwnd),
        WM_DPICHANGED => on_dpi_changed(state, hwnd, lparam),
        WM_MOUSEMOVE => on_mouse_move(state, hwnd, lparam),
        WM_MOUSELEAVE => {
            state.tracking.set(false);
            state.emit(SheetEvent::Leave);
        }
        WM_LBUTTONDOWN => {
            state.pressed.set(true);
            // SAFETY: captures the mouse for this window so the release arrives here.
            let _ = unsafe { SetCapture(hwnd) };
        }
        WM_LBUTTONUP => on_button_up(state, hwnd, lparam),
        WM_CAPTURECHANGED => state.pressed.set(false),
        WM_KEYDOWN => on_key_down(state, hwnd, wparam, lparam),
        WM_CHAR => on_char(state, wparam),
        WM_CLOSE => {
            on_close(state);
            return Some(LRESULT(0));
        }
        WM_APP_FLUSH => state.flush(),
        // The render thread owns every pixel; Windows must not paint or erase over it.
        WM_ERASEBKGND => return Some(LRESULT(1)),
        WM_PAINT => {
            // SAFETY: a null rectangle validates the whole client area of this window.
            let _ = unsafe { ValidateRect(Some(hwnd), None) };
            return Some(LRESULT(0));
        }
        _ => {}
    }
    None
}

/// The window procedure of class `Plith.Settings`.
pub(super) unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_NCCREATE {
        // SAFETY: for WM_NCCREATE `lparam` points at the CREATESTRUCTW; `lpCreateParams` is the
        // pointer from `Rc::into_raw` that `open` passed, now owned by the window.
        unsafe {
            let create = &*(lparam.0 as *const CREATESTRUCTW);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
            let state = &*create.lpCreateParams.cast::<State>();
            state.attached.set(true);
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        }
    }
    // SAFETY: GWLP_USERDATA holds 0 or the pointer stored at WM_NCCREATE.
    let raw = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *const State;
    if raw.is_null() {
        // SAFETY: forwards the unchanged message to the default procedure.
        return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
    }
    // SAFETY: the window owns one strong count; this one keeps the state alive even if a callback
    // drops the handle while the message is processed.
    let state = unsafe {
        Rc::increment_strong_count(raw);
        Rc::from_raw(raw)
    };
    let mut result = None;
    let outcome = guard("settings window procedure", || {
        result = handle(&state, hwnd, msg, wparam, lparam);
    });
    if let Err(message) = outcome {
        state.fail(message);
    }
    if msg == WM_NCDESTROY {
        clear_owner(hwnd.0 as isize);
        // SAFETY: the window is gone; clear the slot and release the count it owned.
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            drop(Rc::from_raw(raw));
        }
    }
    // SAFETY: forwards the message when it was not fully handled above.
    result.unwrap_or_else(|| unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) })
}

/// Destroys the window if it is still alive.
pub(super) fn destroy(raw: isize) {
    let hwnd = hwnd_of(raw);
    // SAFETY: IsWindow accepts any handle value; DestroyWindow runs only on a live window of this
    // thread.
    if unsafe { IsWindow(Some(hwnd)) }.as_bool() {
        let _ = unsafe { DestroyWindow(hwnd) };
    }
}
