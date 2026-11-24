//! A drag out of the island: a wagon pressed and pulled past the slop becomes an OLE drag of the
//! genuine shell data object of its file, offered as copy or link, never move.
//!
//! `DoDragDrop` is modal: it pumps the thread's messages until the button goes up. The caller
//! must run it from the handler of a real mouse event while the button is down (a press it never
//! received hangs it or ends it at once). The shell queues the events that come in meanwhile
//! and the island's own drop target answers `DROPEFFECT_NONE` for the drag (`dragging_out`).

mod data;
mod source;

use crate::handles::hwnd_of;
use crate::window::state::{Inner, current};
use source::DropSource;
use std::path::{Path, PathBuf};
use std::time::Instant;
use windows::Win32::Foundation::{DRAGDROP_S_CANCEL, DRAGDROP_S_DROP, LPARAM, POINT, WPARAM};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, IDataObject};
use windows::Win32::System::Ole::{
    DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_LINK, DROPEFFECT_NONE, DoDragDrop, IDropSource,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, ReleaseCapture, VK_LBUTTON};
use windows::Win32::UI::Shell::{CLSID_DragDropHelper, IDragSourceHelper};
use windows::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, PostMessageW, WM_CLOSE, WindowFromPoint,
};

/// What a drag out did, for the `drag` log line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DragOutReport {
    /// How many files were offered.
    pub offered: usize,
    /// How many were skipped because the file was gone.
    pub missing: usize,
    /// `copy`, `link`, `none` (cancelled, or refused by the target), `busy` (a drag out was
    /// already running), `no-button` (the button was not down), `empty` (nothing left to offer)
    /// or `error`.
    pub effect: &'static str,
    /// How long the call took, in milliseconds.
    pub ms: u32,
    /// The pointer is over the island when the drag ended: no leave will come to say so.
    pub inside: bool,
}

/// True when the pointer is over the window `hwnd` (its region, so the island shape).
fn pointer_inside(hwnd: isize) -> bool {
    let mut at = POINT::default();
    // SAFETY: `at` is a valid out value for the call.
    let got = unsafe { GetCursorPos(&raw mut at) }.is_ok();
    // SAFETY: plain hit test at a screen point.
    got && unsafe { WindowFromPoint(at) } == hwnd_of(hwnd)
}

/// Sets the island's own drag flag for the length of the call, whatever way it ends.
struct OwnDrag<'a>(&'a Inner);

impl<'a> OwnDrag<'a> {
    fn begin(inner: &'a Inner) -> Self {
        inner.dragging_out.set(true);
        Self(inner)
    }
}

impl Drop for OwnDrag<'_> {
    fn drop(&mut self) {
        self.0.dragging_out.set(false);
        // OLE owned the mouse: the tracking, the last point and the press are stale.
        self.0.tracking.set(false);
        self.0.last_point.set(None);
        self.0.press.cancel();
        // Released over the island: ask for the leave now, no first move may come to arm it.
        if pointer_inside(self.0.visual.get()) {
            crate::window::mouse::rearm_leave(self.0);
        }
        // A close that came in during the modal loop waits until the drag is over, so the
        // window is never destroyed under `DoDragDrop`.
        if self.0.close_pending.replace(false) {
            // SAFETY: PostMessageW accepts any handle value; a stale one only makes it fail.
            let _ = unsafe {
                PostMessageW(
                    Some(hwnd_of(self.0.visual.get())),
                    WM_CLOSE,
                    WPARAM(0),
                    LPARAM(0),
                )
            };
        }
    }
}

pub(crate) fn effect_word(effect: DROPEFFECT) -> &'static str {
    if effect & DROPEFFECT_LINK == DROPEFFECT_LINK {
        "link"
    } else if effect & DROPEFFECT_COPY == DROPEFFECT_COPY {
        "copy"
    } else {
        "none"
    }
}

/// The logical left button, as the handled message queue sees it. `GetAsyncKeyState` reads the
/// physical button, which is the right one for a user with swapped buttons, and it reads zero
/// while another process holds the foreground (always the case for this no-activate window).
fn left_button_down() -> bool {
    // SAFETY: plain state read with a virtual key code.
    let state = unsafe { GetKeyState(i32::from(VK_LBUTTON.0)) };
    // The high bit (a negative value) is the "down now" flag.
    state < 0
}

/// Lets the shell build the drag image from the data object (a null window asks it to).
fn prepare_image(data: &IDataObject) {
    // SAFETY: plain COM creation and call on live objects; the image is a courtesy.
    unsafe {
        if let Ok(helper) = CoCreateInstance::<_, IDragSourceHelper>(
            &CLSID_DragDropHelper,
            None,
            CLSCTX_INPROC_SERVER,
        ) {
            let _ = helper.InitializeFromWindow(None, None, data);
        }
    }
}

/// Starts the drag of `paths` and returns when it ends. Files that no longer exist are
/// skipped. Call it only on the UI thread, from the handler of a mouse event, with the left
/// button down.
#[must_use]
pub fn start(paths: &[PathBuf]) -> DragOutReport {
    let began = Instant::now();
    let report = |offered, missing, effect| DragOutReport {
        offered,
        missing,
        effect,
        // Allowed: a drag lasting longer than u32 milliseconds (49 days) is not a case.
        #[allow(clippy::cast_possible_truncation)]
        ms: began.elapsed().as_millis() as u32,
        inside: false,
    };
    let Some(inner) = current() else {
        return report(0, paths.len(), "error");
    };
    if inner.dragging_out.get() {
        return report(0, 0, "busy");
    }
    let live: Vec<&Path> = paths
        .iter()
        .map(PathBuf::as_path)
        .filter(|p| p.exists())
        .collect();
    let missing = paths.len() - live.len();
    if live.is_empty() {
        return report(0, missing, "empty");
    }
    if !left_button_down() {
        return report(live.len(), missing, "no-button");
    }
    let Ok(data_object) = data::shell_data_object(&live) else {
        return report(live.len(), missing, "error");
    };
    prepare_image(&data_object);
    let source: IDropSource = DropSource.into();
    // OLE takes its own capture for the drag.
    // SAFETY: releasing a capture this thread does not hold only fails.
    let _ = unsafe { ReleaseCapture() };
    let guard = OwnDrag::begin(&inner);
    let mut effect = DROPEFFECT_NONE;
    // SAFETY: both objects are live for the call and `effect` is a writable out value.
    let status = unsafe {
        DoDragDrop(
            &data_object,
            &source,
            DROPEFFECT_COPY | DROPEFFECT_LINK,
            &raw mut effect,
        )
    };
    drop(guard);
    let word = match status {
        s if s == DRAGDROP_S_DROP => effect_word(effect),
        s if s == DRAGDROP_S_CANCEL => "none",
        _ => "error",
    };
    DragOutReport {
        inside: pointer_inside(inner.visual.get()),
        ..report(live.len(), missing, word)
    }
}
