//! Applies the latest drawn shape as the window region and keeps timing statistics.

use super::region_px;
use crate::handles::hwnd_of;
use crate::window::state::Inner;
use plith_core::geometry::{IslandShape, cover};
use plith_core::shapes::REST;
use std::time::Instant;
use windows::Win32::Foundation::E_FAIL;
use windows::Win32::Graphics::Gdi::{CreateRoundRectRgn, DeleteObject, HGDIOBJ, SetWindowRgn};

/// Outset that keeps the anti-aliased edge of the island inside the hit region. At 1 px the
/// region (a GDI approximation of the arc) clipped the drawn edge and Berke saw jagged corners
/// (Task 10, 2026-11-14); 3 px keeps the region boundary outside the anti-aliasing ramp.
const OUTSET_PX: i32 = 3;

fn set_region(inner: &Inner, shape: IslandShape) -> windows::core::Result<()> {
    let r = region_px(shape, &inner.layout, OUTSET_PX);
    // SAFETY: every argument is a plain integer, so no pointer is read; the call returns a new region that we own until SetWindowRgn takes it.
    let rgn = unsafe { CreateRoundRectRgn(r.left, r.top, r.right, r.bottom, r.ellipse, r.ellipse) };
    if rgn.is_invalid() {
        // CreateRoundRectRgn documents no GetLastError, so the thread error may be stale.
        return Err(windows::core::Error::from(E_FAIL));
    }
    let hwnd = hwnd_of(inner.visual.get());
    // SAFETY: `hwnd` is a window of this thread. On success the system owns `rgn`.
    if unsafe { SetWindowRgn(hwnd, Some(rgn), false) } == 0 {
        // SAFETY: the call failed, so we still own `rgn` and delete it once.
        let _ = unsafe { DeleteObject(HGDIOBJ(rgn.0)) };
        // SetWindowRgn documents no GetLastError, so the thread error may be stale.
        return Err(windows::core::Error::from(E_FAIL));
    }
    Ok(())
}

/// Applies the region for the newest frame in the cell (or the resting shape when none was
/// posted yet), recording the UI-thread cost. A failure is counted in `hit_errors`.
///
/// The region covers the frame before the newest one as well: with a frame queue the display
/// can still show it when the region changes, and the region clips the drawn content (ADR 0004).
pub(crate) fn apply_latest(inner: &Inner) {
    let shape = inner
        .shapes
        .latest()
        .map_or(REST, |frame| cover(frame.shape, frame.previous));
    let start = Instant::now();
    let result = set_region(inner, shape);
    let ms = start.elapsed().as_secs_f64() * 1000.0;
    let mut stats = inner.stats.get();
    stats.count += 1;
    stats.total_ms += ms;
    stats.max_ms = stats.max_ms.max(ms);
    inner.stats.set(stats);
    if result.is_err() {
        inner.hit_errors.set(inner.hit_errors.get() + 1);
    }
}
