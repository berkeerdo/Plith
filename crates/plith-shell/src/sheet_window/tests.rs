//! A hidden-window smoke test: the window is created without `WS_VISIBLE` and never shown or
//! activated, so nothing appears on screen.

use super::*;
use std::cell::RefCell;
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, MSG, PM_REMOVE, PeekMessageW, SendMessageW, TranslateMessage, WM_CLOSE,
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
    let events: Rc<RefCell<Vec<SheetEvent>>> = Rc::default();
    let sink = Rc::clone(&events);
    let window = SheetWindow::open_with(
        0,
        "Plith test",
        Box::new(move |event| sink.borrow_mut().push(event)),
        false,
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
    window.close();
}
