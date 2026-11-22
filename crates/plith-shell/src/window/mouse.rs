//! Pointer messages: a left-button press inside the drawn island is reported, then becomes a
//! click at the release point (standard button semantics with capture) or, once the pointer
//! passes the drag slop, a drag that ends in a release. Leave tracking turns the pointer
//! leaving the window region into a leave.

mod press;

use super::ShellEvent;
use super::state::Inner;
use crate::handles::hwnd_of;
use plith_core::geometry::IslandShape;
use plith_core::layout::SurfaceLayout;
pub(crate) use press::{PressEnd, PressRecord};
use windows::Win32::Foundation::LPARAM;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    ReleaseCapture, SetCapture, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
};

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
    let (ix, iy) = island_point_unclamped(layout, shape, (x, y));
    shape.contains(ix, iy).then_some((ix, iy))
}

/// The client pixel point `(x, y)` as island DIP with no `contains` check: a captured pointer
/// keeps reporting while it is outside the shape or the window.
#[must_use]
pub(crate) fn island_point_unclamped(
    layout: &SurfaceLayout,
    shape: IslandShape,
    (x, y): (f64, f64),
) -> (f64, f64) {
    let scale = layout.scale;
    let (ox, oy) = layout.island_origin(shape);
    (x / scale - ox, y / scale - oy)
}

/// The client pixel point packed in a mouse message's `lparam`.
fn client_point(lparam: LPARAM) -> (f64, f64) {
    // Allowed: the low and high words are signed 16-bit client coordinates by definition.
    #[allow(clippy::cast_possible_truncation)]
    let (x, y) = (lparam.0 as i16, (lparam.0 >> 16) as i16);
    (f64::from(x), f64::from(y))
}

/// The island point of a mouse message, or `None` outside the drawn shape or before the first
/// shape was published.
fn message_point(inner: &Inner, lparam: LPARAM) -> Option<(f64, f64)> {
    let frame = inner.shapes.latest()?;
    island_point(&inner.layout(), frame.shape, client_point(lparam))
}

/// `WM_LBUTTONDOWN`: records a press inside the shape. Returns `true` when the caller must take
/// the mouse capture so the matching release reaches this window.
pub(crate) fn note_down(inner: &Inner, lparam: LPARAM) -> bool {
    let point = message_point(inner, lparam);
    let counts = inner.press.down(point);
    if let Some((x, y)) = point {
        inner.emit(ShellEvent::Press { x, y });
    }
    counts
}

/// The unclamped island point of a mouse message; before the first shape was published (so no
/// press can exist) the origin.
fn raw_point(inner: &Inner, lparam: LPARAM) -> (f64, f64) {
    inner.shapes.latest().map_or((0.0, 0.0), |frame| {
        island_point_unclamped(&inner.layout(), frame.shape, client_point(lparam))
    })
}

/// `WM_LBUTTONUP`: a press that never passed the slop emits a click at the release point when
/// the release lies inside the drawn shape; a drag emits a release at the (unclamped) release
/// point, then a leave when that point is outside the shape (the window region is the shape, so
/// no later `WM_MOUSELEAVE` may come), whether or not one arrived during the capture.
/// A click may expand the island, so the next move asks for leave tracking again: a tracking
/// request the system cancelled can then never leave the island open with no leave to come.
pub(crate) fn note_up(inner: &Inner, lparam: LPARAM) {
    match inner
        .press
        .up(message_point(inner, lparam), raw_point(inner, lparam))
    {
        PressEnd::Nothing => {}
        PressEnd::Click((x, y)) => {
            inner.tracking.set(false);
            inner.emit(ShellEvent::Click { x, y });
        }
        PressEnd::Release { at: (x, y), leave } => {
            inner.tracking.set(false);
            inner.emit(ShellEvent::Release { x, y });
            if leave {
                emit_leave(inner);
            }
        }
    }
}

/// `WM_CAPTURECHANGED`: a drag whose capture was lost ends at its last point, followed by a
/// leave when one arrived meanwhile or that point lies outside the shape. A press that never
/// dragged is forgotten.
pub(crate) fn note_capture_lost(inner: &Inner) {
    let shape = inner.shapes.latest().map(|frame| frame.shape);
    let inside = |(x, y): (f64, f64)| shape.is_none_or(|shape| shape.contains(x, y));
    if let Some(((x, y), leave)) = inner.press.lost(inside) {
        inner.tracking.set(false);
        inner.emit(ShellEvent::Release { x, y });
        if leave {
            emit_leave(inner);
        }
    }
}

/// `WM_LBUTTONDOWN` with capture: a counted press makes this window capture the mouse.
pub(crate) fn on_button_down(inner: &Inner, lparam: LPARAM) {
    if note_down(inner, lparam) {
        // SAFETY: names the notch window of this thread; capture does not activate it, and the
        // previous capture owner (none in practice) is not used.
        let _ = unsafe { SetCapture(hwnd_of(inner.visual.get())) };
    }
}

/// `WM_LBUTTONUP` with capture: emits the click (if any), then releases the capture.
pub(crate) fn on_button_up(inner: &Inner, lparam: LPARAM) {
    note_up(inner, lparam);
    release_capture();
}

/// Gives the capture back. A failure (we did not hold it) changes nothing.
fn release_capture() {
    // SAFETY: plain call; releasing a capture this thread does not hold is a no-op error.
    let _ = unsafe { ReleaseCapture() };
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

/// Notes the pointer at `lparam`: a point inside the drawn shape that differs from the last one
/// emitted becomes a `Move`. No timer and no repeat for a pointer at rest.
pub(crate) fn note_point(inner: &Inner, lparam: LPARAM) {
    note_move_point(inner, lparam);
    // A pressed pointer reports every new point past the slop, inside the shape or not.
    if inner.shapes.latest().is_some()
        && let Some((x, y)) = inner.press.moved(raw_point(inner, lparam))
    {
        inner.emit(ShellEvent::Drag { x, y });
    }
}

/// The `Move` half of [`note_point`]: a point inside the drawn shape.
fn note_move_point(inner: &Inner, lparam: LPARAM) {
    let Some(point) = message_point(inner, lparam) else {
        return;
    };
    if inner.last_point.replace(Some(point)) != Some(point) {
        inner.emit(ShellEvent::Move {
            x: point.0,
            y: point.1,
        });
    }
}

/// Notes that the pointer left the window region and emits `Leave`, unless a drag is on: then
/// the drag goes on and the leave is emitted right after its release.
pub(crate) fn note_leave(inner: &Inner) {
    if inner.press.leave() {
        return;
    }
    emit_leave(inner);
}

/// Emits `Leave` and forgets the tracking state.
fn emit_leave(inner: &Inner) {
    inner.tracking.set(false);
    inner.last_point.set(None);
    inner.emit(ShellEvent::Leave);
}

/// `WM_MOUSEMOVE`: on the first move since a leave, asks for one `WM_MOUSELEAVE`; a new point
/// inside the shape is reported as a `Move`.
pub(crate) fn on_move(inner: &Inner, lparam: LPARAM) {
    let first = note_move(inner);
    note_point(inner, lparam);
    if !first {
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
