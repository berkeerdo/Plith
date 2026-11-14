//! Hit-region math for the three hit-testing candidates (see the spikes doc).

pub(crate) mod apply;
pub(crate) mod mask;

use plith_core::geometry::IslandShape;
use plith_core::layout::SurfaceLayout;

/// Arguments for `CreateRoundRectRgn`, in client pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegionPx {
    /// Left edge.
    pub left: i32,
    /// Top edge.
    pub top: i32,
    /// Right edge, exclusive.
    pub right: i32,
    /// Bottom edge, exclusive.
    pub bottom: i32,
    /// Corner ellipse width and height (twice the radius).
    pub ellipse: i32,
}

#[allow(clippy::cast_possible_truncation)]
fn px(dip: f64, scale: f64) -> i32 {
    (dip * scale).round() as i32
}

/// The island's rounded rectangle in client pixels, grown by `outset_px` on every side so the
/// anti-aliased edge is not clipped. The corner radius grows with the outset.
#[must_use]
pub fn region_px(shape: IslandShape, layout: &SurfaceLayout, outset_px: i32) -> RegionPx {
    let (ox, oy) = layout.island_origin(shape);
    let scale = layout.scale;
    let radius = shape.radius.min(shape.width / 2.0).min(shape.height / 2.0);
    RegionPx {
        left: px(ox, scale) - outset_px,
        top: px(oy, scale) - outset_px,
        right: px(ox + shape.width, scale) + outset_px,
        bottom: px(oy + shape.height, scale) + outset_px,
        ellipse: 2 * (px(radius, scale) + outset_px),
    }
}

#[cfg(test)]
mod tests;
