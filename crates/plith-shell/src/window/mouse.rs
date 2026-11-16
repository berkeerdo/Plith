//! Pointer messages: a left-button release inside the drawn island becomes a click at an
//! island point, and leave tracking turns the pointer leaving the window region into a leave.

use super::ShellEvent;
use super::state::Inner;
use crate::handles::hwnd_of;
use plith_core::geometry::IslandShape;
use plith_core::layout::SurfaceLayout;
use windows::Win32::Foundation::LPARAM;
use windows::Win32::UI::Input::KeyboardAndMouse::{TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent};

/// `WM_MOUSELEAVE`; its constant lives in the large `Win32_UI_Controls` feature, so it is
/// spelled here (winuser.h: `0x02A3`).
pub(crate) const WM_MOUSELEAVE: u32 = 0x02A3;

/// The client pixel point `(x, y)` as island DIP (relative to the island's top-left), or
/// `None` when it lies outside the drawn shape.
#[must_use]
pub(crate) fn island_point(
    layout: &SurfaceLayout,
    shape: IslandShape,
    (x, y): (f64, f64),
) -> Option<(f64, f64)> {
    let scale = layout.scale;
    let (ox, oy) = layout.island_origin(shape);
    let (ix, iy) = (x / scale - ox, y / scale - oy);
    shape.contains(ix, iy).then_some((ix, iy))
}

/// The client pixel point packed in a mouse message's `lparam`.
fn client_point(lparam: LPARAM) -> (f64, f64) {
    // Allowed: the low and high words are signed 16-bit client coordinates by definition.
    #[allow(clippy::cast_possible_truncation)]
    let (x, y) = (lparam.0 as i16, (lparam.0 >> 16) as i16);
    (f64::from(x), f64::from(y))
}

/// `WM_LBUTTONUP`: emits a click at the island point when it lies inside the drawn shape.
/// A click may expand the island, so the next move asks for leave tracking again: a tracking
/// request the system cancelled can then never leave the island open with no leave to come.
pub(crate) fn on_click(inner: &Inner, lparam: LPARAM) {
    let Some(frame) = inner.shapes.latest() else {
        return;
    };
    if let Some((x, y)) = island_point(&inner.layout, frame.shape, client_point(lparam)) {
        inner.tracking.set(false);
        inner.emit(ShellEvent::Click { x, y });
    }
}

/// Notes a pointer move. The first move since the last leave emits `Enter` and returns `true`:
/// the caller must then ask for the next leave.
pub(crate) fn note_move(inner: &Inner) -> bool {
    if inner.tracking.replace(true) {
        return false;
    }
    inner.emit(ShellEvent::Enter);
    true
}

/// Notes that the pointer left the window region and emits `Leave`.
pub(crate) fn note_leave(inner: &Inner) {
    inner.tracking.set(false);
    inner.emit(ShellEvent::Leave);
}

/// `WM_MOUSEMOVE`: on the first move since a leave, asks for one `WM_MOUSELEAVE`.
pub(crate) fn on_move(inner: &Inner) {
    if !note_move(inner) {
        return;
    }
    let mut track = TRACKMOUSEEVENT {
        cbSize: u32::try_from(size_of::<TRACKMOUSEEVENT>()).unwrap_or(0),
        dwFlags: TME_LEAVE,
        hwndTrack: hwnd_of(inner.visual.get()),
        dwHoverTime: 0,
    };
    // SAFETY: `track` is a fully initialised TRACKMOUSEEVENT that lives for the whole call and
    // names the notch window of this thread. A failure only means no leave is reported, so the
    // flag is cleared to ask again on the next move.
    if unsafe { TrackMouseEvent(&raw mut track) }.is_err() {
        inner.tracking.set(false);
    }
}
