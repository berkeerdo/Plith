//! Pointer messages: a left-button press and release inside the drawn island become a click at
//! the release point (standard button semantics with capture), and leave tracking turns the
//! pointer leaving the window region into a leave.

use super::ShellEvent;
use super::state::Inner;
use crate::handles::hwnd_of;
use plith_core::geometry::IslandShape;
use plith_core::layout::SurfaceLayout;
use std::cell::Cell;
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

/// The press half of a click: the island point where the left button went down inside the
/// shape, kept until the release, a lost capture or a leave.
#[derive(Debug, Default)]
pub(crate) struct PressRecord(Cell<Option<(f64, f64)>>);

impl PressRecord {
    /// Records a press at `point` (`None`: outside the shape, which clears any record).
    /// Returns `true` when the press counts and the window should take the capture.
    pub(crate) fn down(&self, point: Option<(f64, f64)>) -> bool {
        self.0.set(point);
        point.is_some()
    }

    /// Ends the press: the click point when a press was recorded and the release `point` lies
    /// inside the shape, else `None`. The record is always cleared.
    pub(crate) fn up(&self, point: Option<(f64, f64)>) -> Option<(f64, f64)> {
        self.0.take().and(point)
    }

    /// Forgets the press (capture lost or pointer left).
    pub(crate) fn cancel(&self) {
        self.0.set(None);
    }
}

/// The island point of a mouse message, or `None` outside the drawn shape or before the first
/// shape was published.
fn message_point(inner: &Inner, lparam: LPARAM) -> Option<(f64, f64)> {
    let frame = inner.shapes.latest()?;
    island_point(&inner.layout, frame.shape, client_point(lparam))
}

/// `WM_LBUTTONDOWN`: records a press inside the shape. Returns `true` when the caller must take
/// the mouse capture so the matching release reaches this window.
pub(crate) fn note_down(inner: &Inner, lparam: LPARAM) -> bool {
    inner.press.down(message_point(inner, lparam))
}

/// `WM_LBUTTONUP`: emits a click at the release point when the press went down on the island
/// and the release lies inside the drawn shape. A click may expand the island, so the next move
/// asks for leave tracking again: a tracking request the system cancelled can then never leave
/// the island open with no leave to come.
pub(crate) fn note_up(inner: &Inner, lparam: LPARAM) {
    if let Some((x, y)) = inner.press.up(message_point(inner, lparam)) {
        inner.tracking.set(false);
        inner.emit(ShellEvent::Click { x, y });
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

/// Notes that the pointer left the window region and emits `Leave`.
pub(crate) fn note_leave(inner: &Inner) {
    inner.tracking.set(false);
    inner.press.cancel();
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
