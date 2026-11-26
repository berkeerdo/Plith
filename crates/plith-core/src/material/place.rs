//! Where the Halka rasters land on the island (`docs/design/themes/halka/README.md`): every
//! picture is placed relative to the source centre, which sits at `(width / 2, 33)` DIP of the
//! island. All values are DIP; the files' pixel sizes are these times their scale class.

use crate::geometry::IslandShape;
use crate::shapes::REST_HIDDEN;

/// The source centre's height below the island's top edge.
pub const SOURCE_Y: f64 = 33.0;
/// The fabric's DIP width (its 1x file is this many pixels wide).
pub const FABRIC_W: f64 = 640.0;
/// The source centre inside the fabric raster.
pub const FABRIC_ANCHOR: (f64, f64) = (320.0, 33.23);
/// The top-left of each ripple ring relative to the source centre, innermost first.
pub const RING_AT: [(f64, f64); 3] = [(-109.0, -8.23), (-149.0, 21.77), (-177.0, 61.77)];
/// The top-left of the source disc relative to the source centre, before its lift.
pub const DISC_AT: (f64, f64) = (-69.0, -33.23);
/// The pill's straight middle starts this far from either end of its raster.
pub const PILL_CAP: f64 = 32.0;
/// The pill raster's own height: a shorter island crops it; a taller one shows it at this
/// height on top and the opaque base below (it is never stretched down).
pub const PILL_H: f64 = 32.0;

/// How much of the pill's two caps shows on an island `height` tall: none on the Hidden sliver
/// (their rounded ends carry a light rim that would show as two specks in 3 DIP), all from the
/// pill's own height on, linear between. Under a faded cap the pill's straight middle shows.
#[must_use]
pub fn cap_opacity(height: f64) -> f64 {
    let low = REST_HIDDEN.height;
    ((height - low) / (PILL_H - low)).clamp(0.0, 1.0)
}

/// The source centre of an island of `shape` whose top-left is `origin`.
#[must_use]
pub fn source_point(shape: IslandShape, origin: (f64, f64)) -> (f64, f64) {
    (origin.0 + shape.width / 2.0, origin.1 + SOURCE_Y)
}

/// The reveal circle's radius: `reveal` (0..1) of the distance from the source centre to the
/// island's farthest corner.
#[must_use]
pub fn reveal_radius(shape: IslandShape, reveal: f64) -> f64 {
    let dx = shape.width / 2.0;
    let dy = SOURCE_Y.max(shape.height - SOURCE_Y);
    reveal.clamp(0.0, 1.0) * dx.hypot(dy)
}

/// One horizontal slice of a three-slice draw: the source span in the raster and the
/// destination span on the island, both `(left, width)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Slice {
    /// Source left edge and width, in the raster's DIP.
    pub src: (f64, f64),
    /// Destination left edge and width.
    pub dst: (f64, f64),
}

/// The left cap, the stretched middle and the right cap of a raster `art_w` wide drawn over
/// `width` from `left`. A destination narrower than two caps shrinks the caps to half each.
#[must_use]
pub fn three_slice(art_w: f64, left: f64, width: f64) -> [Slice; 3] {
    let cap = PILL_CAP.min(art_w / 2.0).min(width / 2.0).max(0.0);
    let src_cap = PILL_CAP.min(art_w / 2.0);
    [
        Slice {
            src: (0.0, src_cap),
            dst: (left, cap),
        },
        Slice {
            src: (src_cap, art_w - 2.0 * src_cap),
            dst: (left + cap, width - 2.0 * cap),
        },
        Slice {
            src: (art_w - src_cap, src_cap),
            dst: (left + width - cap, cap),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shapes::{OVERSHOOT, PAGE_MAX_HEIGHT, PAGE_MAX_WIDTH, REST};

    #[test]
    fn the_caps_fade_out_toward_the_hidden_sliver() {
        assert!(cap_opacity(REST_HIDDEN.height).abs() < 1e-12);
        assert!(cap_opacity(0.0).abs() < 1e-12);
        assert!((cap_opacity(REST.height) - 1.0).abs() < 1e-12);
        assert!((cap_opacity(300.0) - 1.0).abs() < 1e-12);
        let mid = REST_HIDDEN.height.midpoint(PILL_H);
        assert!((cap_opacity(mid) - 0.5).abs() < 1e-12);
    }

    fn shape(width: f64, height: f64) -> IslandShape {
        IslandShape {
            width,
            height,
            radius: 24.0,
        }
    }

    #[test]
    fn the_source_sits_top_centre() {
        assert_eq!(source_point(REST, (10.0, 4.0)), (110.0, 37.0));
    }

    #[test]
    fn full_reveal_reaches_the_far_corner() {
        let s = shape(600.0, 433.0);
        assert!((reveal_radius(s, 1.0) - 300.0_f64.hypot(400.0)).abs() < 1e-9);
        assert!((reveal_radius(s, 0.5) - 250.0).abs() < 1e-9);
        assert!(reveal_radius(s, -1.0).abs() < 1e-12);
    }

    #[test]
    fn a_short_island_measures_to_its_top_corner() {
        // The source is 33 below the top: the top corners are the far ones at a 3 DIP sliver.
        let r = reveal_radius(REST_HIDDEN, 1.0);
        assert!((r - 100.0_f64.hypot(33.0)).abs() < 1e-9);
    }

    #[test]
    fn the_largest_island_stays_near_the_fabric() {
        // The fabric is cut for 620 x 420; the overshoot passes it by less than its mirror
        // reach (a mirrored band is never wider than the raster itself).
        let (w, h) = (PAGE_MAX_WIDTH * OVERSHOOT, PAGE_MAX_HEIGHT * OVERSHOOT);
        assert!(w - FABRIC_W < FABRIC_W && h - 420.0 < 420.0);
    }

    #[test]
    fn three_slice_keeps_the_caps_and_stretches_the_middle() {
        let [l, m, r] = three_slice(244.0, 5.0, 200.0);
        assert_eq!(l.src, (0.0, 32.0));
        assert_eq!(l.dst, (5.0, 32.0));
        assert_eq!(m.src, (32.0, 180.0));
        assert_eq!(m.dst, (37.0, 136.0));
        assert_eq!(r.src, (212.0, 32.0));
        assert_eq!(r.dst, (173.0, 32.0));
    }

    #[test]
    fn a_narrow_island_shrinks_the_caps() {
        let [l, m, r] = three_slice(244.0, 0.0, 40.0);
        assert_eq!(l.dst, (0.0, 20.0));
        assert_eq!(m.dst, (20.0, 0.0));
        assert_eq!(r.dst, (20.0, 20.0));
    }
}
