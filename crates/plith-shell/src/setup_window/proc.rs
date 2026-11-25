//! The installer window's state and window procedure, behind `GWLP_USERDATA`.

use super::{SetupEvent, WM_SETUP_WAKE};
use crate::handles::hwnd_of;
use crate::window::guard;
use plith_core::setup::{SetupAction, SetupTarget, setup_hit};
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{ScreenToClient, ValidateRect};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture};
use windows::Win32::UI::WindowsAndMessaging::{
    CREATESTRUCTW, DefWindowProcW, DestroyWindow, GWLP_USERDATA, GetClientRect, GetWindowLongPtrW,
    HTCAPTION, HTCLIENT, IsWindow, SWP_NOACTIVATE, SWP_NOZORDER, SetWindowLongPtrW, SetWindowPos,
    WM_CAPTURECHANGED, WM_CLOSE, WM_DPICHANGED, WM_ERASEBKGND, WM_KEYDOWN, WM_LBUTTONDOWN,
    WM_LBUTTONUP, WM_NCCREATE, WM_NCDESTROY, WM_NCHITTEST, WM_NCLBUTTONDBLCLK, WM_PAINT,
};

/// Posted by `open` so events queued while the window was being created are delivered from a
/// later message, never from inside `open`.
pub(super) const WM_APP_FLUSH: u32 = 0x8001;

/// Per-window state, shared by the handle and the window procedure.
pub(super) struct State {
    on_event: RefCell<Box<dyn FnMut(SetupEvent)>>,
    queue: RefCell<VecDeque<SetupEvent>>,
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
    /// True once `WM_NCCREATE` stored the pointer that the window owns.
    pub(super) attached: Cell<bool>,
    last_dpi: Cell<u32>,
    /// The click targets of the screen on show.
    pub(super) targets: RefCell<Vec<SetupTarget>>,
}

impl State {
    pub(super) fn new(on_event: Box<dyn FnMut(SetupEvent)>, on_panic: Box<dyn Fn(String)>) -> Self {
        Self {
            on_event: RefCell::new(on_event),
            queue: RefCell::new(VecDeque::new()),
            ready: Cell::new(false),
            dispatching: Cell::new(false),
            on_panic,
            muted: Cell::new(false),
            panicked: Cell::new(false),
            pressed: Cell::new(false),
            attached: Cell::new(false),
            last_dpi: Cell::new(0),
            targets: RefCell::new(Vec::new()),
        }
    }

    /// Queues `event`; delivers it at once unless `open` is still running.
    fn emit(&self, event: SetupEvent) {
        self.queue.borrow_mut().push_back(event);
        if self.ready.get() {
            self.flush();
        }
    }

    /// Delivers every queued event, in order. Events raised by the callback itself join the queue
    /// and are delivered by this same loop, so the callback is never entered twice at once.
    fn flush(&self) {
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
            if let Err(message) = guard("setup window callback", || callback(event)) {
                drop(callback);
                self.fail(message);
            }
        }
        self.dispatching.set(false);
    }

    /// Records a caught panic: reports it once through `on_panic` and mutes the handler.
    fn fail(&self, message: String) {
        self.muted.set(true);
        if self.panicked.replace(true) {
            return;
        }
        // A panic inside the report itself has nowhere left to go.
        let _ = guard("setup window panic report", || (self.on_panic)(message));
    }

    pub(super) fn set_ready(&self) {
        self.ready.set(true);
    }

    pub(super) fn mute(&self) {
        self.muted.set(true);
    }

    /// Emits `Dpi` when the window's DPI differs from the last one reported.
    pub(super) fn emit_dpi(&self, hwnd: HWND) {
        let (_, _, scale) = client_metrics(hwnd);
        // SAFETY: a plain query on a window handle; it returns 0 for a dead window.
        let dpi = unsafe { GetDpiForWindow(hwnd) };
        if self.last_dpi.replace(dpi) != dpi {
            self.emit(SetupEvent::Dpi(scale));
        }
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

fn signed_word(value: isize) -> i16 {
    let word = u16::try_from(value & 0xFFFF).unwrap_or(0);
    i16::from_ne_bytes(word.to_ne_bytes())
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
        state.emit(SetupEvent::Click(
            f64::from(x) / scale,
            f64::from(y) / scale,
        ));
    }
}

fn on_key_down(state: &State, wparam: WPARAM, lparam: LPARAM) {
    // Bit 30 of `lparam` is set for auto-repeat: a held Enter acts once.
    if lparam.0 & (1 << 30) == 0 {
        state.emit(SetupEvent::Key(u32::try_from(wparam.0).unwrap_or(0)));
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
    state.emit_dpi(hwnd);
}

/// The `LRESULT` of a hit-test code.
fn answer(code: u32) -> LRESULT {
    LRESULT(isize::try_from(code).unwrap_or(0))
}

/// `HTCAPTION` where the screen's drag area is, so the system moves the window; else the client.
fn on_hit_test(state: &State, hwnd: HWND, lparam: LPARAM) -> LRESULT {
    let mut at = POINT {
        x: i32::from(signed_word(lparam.0)),
        y: i32::from(signed_word(lparam.0 >> 16)),
    };
    // SAFETY: `at` is a valid in/out point; a failure leaves screen coordinates, which then
    // simply miss the drag area.
    if !unsafe { ScreenToClient(hwnd, &raw mut at) }.as_bool() {
        return answer(HTCLIENT);
    }
    let (_, _, scale) = client_metrics(hwnd);
    let hit = setup_hit(
        &state.targets.borrow(),
        f64::from(at.x) / scale,
        f64::from(at.y) / scale,
    );
    answer(if hit == Some(SetupAction::Drag) {
        HTCAPTION
    } else {
        HTCLIENT
    })
}

fn handle(state: &State, hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
    match msg {
        WM_DPICHANGED => on_dpi_changed(state, hwnd, lparam),
        WM_NCHITTEST => return Some(on_hit_test(state, hwnd, lparam)),
        // The caption area drags; a double click on it must not maximize.
        WM_NCLBUTTONDBLCLK => return Some(LRESULT(0)),
        WM_LBUTTONDOWN => {
            state.pressed.set(true);
            // SAFETY: captures the mouse for this window so the release arrives here.
            let _ = unsafe { SetCapture(hwnd) };
        }
        WM_LBUTTONUP => on_button_up(state, hwnd, lparam),
        WM_CAPTURECHANGED => state.pressed.set(false),
        WM_KEYDOWN => on_key_down(state, wparam, lparam),
        // Only reported: the owner decides, then drops the handle.
        WM_CLOSE => {
            state.emit(SetupEvent::Closed);
            return Some(LRESULT(0));
        }
        WM_SETUP_WAKE => state.emit(SetupEvent::Wake),
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

/// The window procedure of class `Plith.Setup`.
pub(super) unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_NCCREATE {
        // SAFETY: for WM_NCCREATE `lparam` points at the CREATESTRUCTW; `lpCreateParams` is the
        // pointer from `Rc::into_raw` that `create` passed, now owned by the window.
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
    let outcome = guard("setup window procedure", || {
        result = handle(&state, hwnd, msg, wparam, lparam);
    });
    if let Err(message) = outcome {
        state.fail(message);
    }
    if msg == WM_NCDESTROY {
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
