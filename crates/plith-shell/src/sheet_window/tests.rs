//! A hidden-window smoke test: the window is created without `WS_VISIBLE` and never shown or
//! activated, so nothing appears on screen.

use super::*;
use crate::handles::hwnd_of;
use std::cell::RefCell;
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, IsWindow, MSG, PM_REMOVE, PeekMessageW, SendMessageW, TranslateMessage,
    WM_CLOSE, WM_LBUTTONDOWN, WM_LBUTTONUP,
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

#[test]
fn hidden_open_queues_events_and_closes_once() {
    // SAFETY: a plain process setting; a test exe without a DPI manifest needs it so the window
    // scale matches the monitor scale. Failure (already set) is fine.
    let _ = unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
    let events: Rc<RefCell<Vec<SheetEvent>>> = Rc::default();
    let sink = Rc::clone(&events);
    let window = SheetWindow::open_with(
        0,
        "Plith test",
        Box::new(move |event| sink.borrow_mut().push(event)),
        Box::new(|_| {}),
        (false, true),
    )
    .expect("hidden window opens");

    // Nothing is delivered from inside `open`.
    assert!(events.borrow().is_empty(), "events delivered during open");

    let (width, height, scale) = window.client_px();
    assert!((f64::from(width) / scale - 840.0).abs() < 1.0);
    assert!((f64::from(height) / scale - 640.0).abs() < 1.0);

    pump();
    {
        let got = events.borrow();
        assert_eq!(got.len(), 1, "{got:?}");
        assert_eq!(
            got[0],
            SheetEvent::Resized {
                width_px: width,
                height_px: height,
                scale
            }
        );
    }

    // SAFETY: the window is ours and alive; WM_CLOSE is handled by our procedure.
    unsafe { SendMessageW(hwnd_of(window.hwnd()), WM_CLOSE, None, None) };
    assert_eq!(events.borrow().last(), Some(&SheetEvent::Closed));
    assert_eq!(
        events
            .borrow()
            .iter()
            .filter(|e| **e == SheetEvent::Closed)
            .count(),
        1
    );
    // Only reported: the owner destroys the window after stopping its render thread.
    let hwnd = hwnd_of(window.hwnd());
    // SAFETY: IsWindow accepts any handle value.
    assert!(
        unsafe { IsWindow(Some(hwnd)) }.as_bool(),
        "alive after WM_CLOSE"
    );
    window.close();
    // SAFETY: as above.
    assert!(
        !unsafe { IsWindow(Some(hwnd)) }.as_bool(),
        "destroyed by close"
    );
}

fn quiet_state(events: &Rc<RefCell<Vec<SheetEvent>>>, reports: &Rc<RefCell<Vec<String>>>) -> State {
    let sink = Rc::clone(events);
    let reported = Rc::clone(reports);
    State::new(
        Box::new(move |event| {
            sink.borrow_mut().push(event);
            panic!("handler broke");
        }),
        Box::new(move |message| reported.borrow_mut().push(message)),
    )
}

#[test]
fn a_panic_is_reported_once_and_mutes_the_handler() {
    let events = Rc::default();
    let reports = Rc::default();
    let state = quiet_state(&events, &reports);
    state.set_ready();
    state.emit(SheetEvent::Leave);
    state.emit(SheetEvent::Leave);
    state.fail(String::from("a second report"));
    assert_eq!(
        events.borrow().len(),
        1,
        "the panicking handler is not re-entered"
    );
    let reports = reports.borrow();
    assert_eq!(reports.len(), 1, "{reports:?}");
    assert!(reports[0].contains("handler broke"), "{reports:?}");
}

#[test]
fn a_click_is_a_release_inside_after_a_press_here() {
    let events: Rc<RefCell<Vec<SheetEvent>>> = Rc::default();
    let sink = Rc::clone(&events);
    let window = SheetWindow::open_with(
        0,
        "Plith test",
        Box::new(move |event| sink.borrow_mut().push(event)),
        Box::new(|_| {}),
        (false, true),
    )
    .expect("hidden window opens");
    pump();
    let hwnd = hwnd_of(window.hwnd());
    let at = |x: isize, y: isize| LPARAM((y << 16) | (x & 0xFFFF));
    let send = |msg: u32, lparam: LPARAM| {
        // SAFETY: the window is ours and alive.
        unsafe { SendMessageW(hwnd, msg, None, Some(lparam)) };
    };
    let clicks = || {
        events
            .borrow()
            .iter()
            .filter(|e| matches!(e, SheetEvent::Click(..)))
            .count()
    };
    // A release without a press in this window is nothing.
    send(WM_LBUTTONUP, at(10, 10));
    assert_eq!(clicks(), 0);
    // Press then release inside: one click.
    send(WM_LBUTTONDOWN, at(10, 10));
    send(WM_LBUTTONUP, at(10, 10));
    assert_eq!(clicks(), 1);
    // Press then release outside the client area: nothing.
    send(WM_LBUTTONDOWN, at(10, 10));
    send(WM_LBUTTONUP, at(-5, 10));
    assert_eq!(clicks(), 1);
    window.close();
}
