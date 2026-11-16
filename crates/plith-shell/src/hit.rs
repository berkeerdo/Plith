//! Hit-region math: the island (square top, rounded bottom) as the notch window's region (ADR 0004).

pub(crate) mod apply;

use plith_core::geometry::IslandShape;
use plith_core::layout::SurfaceLayout;

/// Arguments for the window region, in client pixels: a round rect over the whole outset
/// rectangle OR'ed with a plain rect from `top` to `square_bottom`, which squares the top corners.
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
    /// Bottom edge, exclusive, of the plain rect that squares the top corners (half the height).
    pub square_bottom: i32,
}

#[allow(clippy::cast_possible_truncation)]
fn px(dip: f64, scale: f64) -> i32 {
    (dip * scale).round() as i32
}

/// The island in client pixels, grown by `outset_px` on the sides and bottom so the anti-aliased
/// edge is not clipped; the top stays flush with the island's top edge (nothing is above the
/// screen edge). The corner radius grows with the outset.
#[must_use]
pub fn region_px(shape: IslandShape, layout: &SurfaceLayout, outset_px: i32) -> RegionPx {
    let (ox, oy) = layout.island_origin(shape);
    let scale = layout.scale;
    let radius = shape.radius.min(shape.width / 2.0).min(shape.height / 2.0);
    let top = px(oy, scale);
    let bottom = px(oy + shape.height, scale) + outset_px;
    RegionPx {
        left: px(ox, scale) - outset_px,
        top,
        right: px(ox + shape.width, scale) + outset_px,
        bottom,
        ellipse: 2 * (px(radius, scale) + outset_px),
        square_bottom: top + (bottom - top) / 2,
    }
}

#[cfg(test)]
mod tests;
