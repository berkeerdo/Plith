//! Where the Halka rasters land on the island (`docs/design/themes/halka/README.md`): every
//! picture is placed relative to the source centre, which sits at `(width / 2, 33)` DIP of the
//! island. All values are DIP; the files' pixel sizes are these times their scale class.

use crate::geometry::IslandShape;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shapes::{OVERSHOOT, PAGE_MAX_HEIGHT, PAGE_MAX_WIDTH, REST, REST_HIDDEN};

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
        // The fabric is cut for 640 x 420; the overshoot passes it by less than its mirror
        // reach (a mirrored band is never wider than the raster itself).
        let (w, h) = (PAGE_MAX_WIDTH * OVERSHOOT, PAGE_MAX_HEIGHT * OVERSHOOT);
        assert!(w - FABRIC_W < FABRIC_W && h - 420.0 < 420.0);
    }
}
