//! A hidden-window smoke test: the window is created without being shown or activated, so
//! nothing appears on screen.

use super::*;
use crate::handles::hwnd_of;
use plith_core::draw::Rect;
use plith_core::setup::SetupAction;
use std::cell::RefCell;
use windows::Win32::Foundation::LRESULT;
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetWindowRect, HTCAPTION, HTCLIENT, IsWindow, MSG, PM_REMOVE, PeekMessageW,
    SendMessageW, TranslateMessage, WM_CLOSE, WM_NCHITTEST,
};

fn pump() {
    let mut msg = MSG::default();
    // SAFETY: `msg` is a valid out pointer; the loop drains this thread's queue.
    while unsafe { PeekMessageW(&raw mut msg, None, 0, 0, PM_REMOVE) }.as_bool() {
        // SAFETY: `msg` was filled by PeekMessageW just above.
        unsafe {
            let _ = TranslateMessage(&raw const msg);
            DispatchMessageW(&raw const msg);
        }
    }
}

fn target(x: f64, y: f64, w: f64, h: f64, action: SetupAction) -> SetupTarget {
    SetupTarget {
        rect: Rect { x, y, w, h },
        action,
    }
}

/// The `LRESULT` of a hit-test code.
fn answer(code: u32) -> LRESULT {
    LRESULT(isize::try_from(code).unwrap_or(0))
}

/// What `WM_NCHITTEST` says for the DIP point `(x, y)` of the window's client area.
fn hit(window: &SetupWindow, x: f64, y: f64) -> LRESULT {
    let hwnd = hwnd_of(window.hwnd());
    let mut rect = RECT::default();
    // SAFETY: `rect` is a valid out pointer and the window is alive.
    let _ = unsafe { GetWindowRect(hwnd, &raw mut rect) };
    let scale = window.client_px().2;
    // Truncation is fine: the point is a few hundred pixels.
    #[allow(clippy::cast_possible_truncation)]
    let (px, py) = (
        rect.left + (x * scale).round() as i32,
        rect.top + (y * scale).round() as i32,
    );
    let word = |v: i32| isize::try_from(v & 0xFFFF).unwrap_or(0);
    let packed = (word(py) << 16) | word(px);
    // SAFETY: the window is ours and alive; WM_NCHITTEST is answered by our procedure.
    unsafe { SendMessageW(hwnd, WM_NCHITTEST, None, Some(LPARAM(packed))) }
}

#[test]
fn hidden_open_sizes_the_window_drags_by_target_and_closes_once() {
    // SAFETY: a plain process setting; a test exe without a DPI manifest needs it so the window
    // scale matches the monitor scale. Failure (already set) is fine.
    let _ = unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
    let events: Rc<RefCell<Vec<SetupEvent>>> = Rc::default();
    let sink = Rc::clone(&events);
    let window = SetupWindow::open_with(
        "Plith setup test",
        Box::new(move |event| sink.borrow_mut().push(event)),
        Box::new(|_| {}),
        false,
    )
    .expect("hidden window opens");
    assert!(events.borrow().is_empty(), "events delivered during open");

    let (width, height, scale) = window.client_px();
    assert!((f64::from(width) / scale - SETUP_W).abs() < 1.0);
    assert!((f64::from(height) / scale - SETUP_H).abs() < 1.0);
    pump();
    assert_eq!(*events.borrow(), [SetupEvent::Dpi(scale)]);

    // Before the screen names its targets nothing drags.
    assert_eq!(hit(&window, 100.0, 100.0), answer(HTCLIENT));
    window.set_targets(vec![
        target(600.0, 10.0, 30.0, 30.0, SetupAction::Close),
        target(0.0, 0.0, SETUP_W, 215.0, SetupAction::Drag),
    ]);
    assert_eq!(hit(&window, 100.0, 100.0), answer(HTCAPTION));
    assert_eq!(hit(&window, 610.0, 20.0), answer(HTCLIENT), "close wins");
    assert_eq!(hit(&window, 100.0, 300.0), answer(HTCLIENT));

    events.borrow_mut().clear();
    wake(window.hwnd());
    pump();
    assert_eq!(*events.borrow(), [SetupEvent::Wake]);

    let hwnd = hwnd_of(window.hwnd());
    // SAFETY: the window is ours and alive; WM_CLOSE is handled by our procedure.
    unsafe { SendMessageW(hwnd, WM_CLOSE, None, None) };
    assert_eq!(events.borrow().last(), Some(&SetupEvent::Closed));
    // Only reported: the owner drops the handle.
    // SAFETY: IsWindow accepts any handle value.
    assert!(
        unsafe { IsWindow(Some(hwnd)) }.as_bool(),
        "alive after WM_CLOSE"
    );
    window.close();
    // SAFETY: as above.
    assert!(
        !unsafe { IsWindow(Some(hwnd)) }.as_bool(),
        "gone after close"
    );
}
