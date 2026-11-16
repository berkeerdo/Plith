//! The island outline: one clockwise path the painter fills and clips with and the window
//! region mirrors. The top edge is square (flush with the screen edge); only the bottom corners
//! are rounded.

use crate::geometry::IslandShape;

/// One segment of a closed outline.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PathSeg {
    /// A straight line to the point.
    Line(f64, f64),
    /// A clockwise quarter circle of `radius` ending at `to`.
    Arc {
        /// End point of the arc.
        to: (f64, f64),
        /// Circle radius.
        radius: f64,
    },
}

/// The outline of `shape` with its top-left at `origin`, as segments after the start point
/// (the origin itself). It runs clockwise: top-right, down the right side, the bottom-right arc,
/// the bottom edge, the bottom-left arc, and back to the start. The last segment ends at the
/// origin. A zero drawn radius gives plain lines and no arcs.
#[must_use]
pub fn island_outline(shape: IslandShape, origin: (f64, f64)) -> Vec<PathSeg> {
    let (ox, oy) = origin;
    let (right, bottom) = (ox + shape.width, oy + shape.height);
    let r = shape.drawn_radius();
    let mut segs = vec![PathSeg::Line(right, oy)];
    if r > 0.0 {
        if shape.height > r {
            segs.push(PathSeg::Line(right, bottom - r));
        }
        segs.push(PathSeg::Arc {
            to: (right - r, bottom),
            radius: r,
        });
        if shape.width > 2.0 * r {
            segs.push(PathSeg::Line(ox + r, bottom));
        }
        segs.push(PathSeg::Arc {
            to: (ox, bottom - r),
            radius: r,
        });
    } else {
        segs.push(PathSeg::Line(right, bottom));
        segs.push(PathSeg::Line(ox, bottom));
    }
    segs.push(PathSeg::Line(ox, oy));
    segs
}

#[cfg(test)]
mod tests;
