//! Fixed overlay surface: sized for the largest page, centred on the monitor.

use crate::geometry::IslandShape;
use crate::shapes::{OVERSHOOT, PAGE_MAX_HEIGHT, PAGE_MAX_WIDTH, SURFACE_MARGIN, TOP_GAP};

/// Integer pixel rectangle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RectPx {
    /// Left edge.
    pub x: i32,
    /// Top edge.
    pub y: i32,
    /// Width.
    pub width: i32,
    /// Height.
    pub height: i32,
}

/// The surface rectangle and the DPI scale it was computed for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceLayout {
    /// Surface rectangle in physical pixels.
    pub surface: RectPx,
    /// Pixels per DIP.
    pub scale: f64,
}

/// Rounds up to whole pixels, ignoring float noise below 1e-9.
#[allow(clippy::cast_possible_truncation)]
fn ceil_px(v: f64) -> i32 {
    (v - 1e-9).ceil() as i32
}

impl SurfaceLayout {
    /// Sizes the surface for the widest and tallest page and centres it on the monitor centre pixel.
    #[must_use]
    pub fn new(monitor: RectPx, work_top_px: i32, scale: f64) -> Self {
        let dip_w = PAGE_MAX_WIDTH * OVERSHOOT + 2.0 * SURFACE_MARGIN;
        let dip_h = TOP_GAP + PAGE_MAX_HEIGHT * OVERSHOOT + SURFACE_MARGIN;
        let mut width = ceil_px(dip_w * scale);
        if width % 2 != 0 {
            width += 1;
        }
        let height = ceil_px(dip_h * scale);
        let centre = monitor.x + monitor.width / 2;
        Self {
            surface: RectPx {
                x: centre - width / 2,
                y: work_top_px,
                width,
                height,
            },
            scale,
        }
    }

    /// Island top-left in DIP relative to the surface: centred, `TOP_GAP` from the top.
    #[must_use]
    pub fn island_origin(&self, shape: IslandShape) -> (f64, f64) {
        let surface_w = f64::from(self.surface.width) / self.scale;
        ((surface_w - shape.width) / 2.0, TOP_GAP)
    }
}

#[cfg(test)]
mod tests;
