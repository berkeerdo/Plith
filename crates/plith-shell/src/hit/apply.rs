//! Applies the latest drawn shape to the active hit candidate and keeps timing statistics.

use super::mask::MaskBuffer;
use super::region_px;
use crate::window::state::Inner;
use plith_core::geometry::IslandShape;
use plith_core::launch::HitMode;
use plith_core::shapes::REST;
use std::time::Instant;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{CreateRoundRectRgn, DeleteObject, HGDIOBJ, SetWindowRgn};

/// Outset that keeps the anti-aliased edge of the island inside the hit region.
const OUTSET_PX: i32 = 1;

fn set_region(inner: &Inner, hwnd: isize, shape: IslandShape) -> windows::core::Result<()> {
    let r = region_px(shape, &inner.layout, OUTSET_PX);
    // SAFETY: plain GDI call returning a new region handle.
    let rgn = unsafe { CreateRoundRectRgn(r.left, r.top, r.right, r.bottom, r.ellipse, r.ellipse) };
    if rgn.is_invalid() {
        return Err(windows::core::Error::from_thread());
    }
    let hwnd = HWND(hwnd as *mut core::ffi::c_void);
    // SAFETY: `hwnd` is a window of this thread. On success the system owns `rgn`.
    if unsafe { SetWindowRgn(hwnd, Some(rgn), false) } == 0 {
        // SAFETY: the call failed, so we still own `rgn` and delete it once.
        let _ = unsafe { DeleteObject(HGDIOBJ(rgn.0)) };
        return Err(windows::core::Error::from_thread());
    }
    Ok(())
}

fn apply_shape(inner: &Inner, shape: IslandShape) -> windows::core::Result<()> {
    match inner.hit_mode {
        HitMode::Region => set_region(inner, inner.visual.get(), shape),
        HitMode::Split => set_region(inner, inner.input.get(), shape),
        HitMode::Mask => {
            let mut slot = inner.mask.borrow_mut();
            if slot.is_none() {
                let surface = inner.layout.surface;
                *slot = Some(MaskBuffer::new(surface.width, surface.height)?);
            }
            match slot.as_mut() {
                Some(mask) => mask.apply(inner.input.get(), shape, &inner.layout),
                None => Ok(()),
            }
        }
    }
}

/// Applies the newest shape in the cell (or the resting shape when none was posted yet),
/// recording the UI-thread cost. A failure is counted in `hit_errors`.
pub(crate) fn apply_latest(inner: &Inner) {
    let shape = inner.shapes.latest().map_or(REST, |frame| frame.shape);
    let start = Instant::now();
    let result = apply_shape(inner, shape);
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
