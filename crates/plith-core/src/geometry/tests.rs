use super::*;

use crate::shapes::{PAGES, REST};

const BIG: IslandShape = PAGES[0];

#[test]
fn contains_rejects_the_bottom_corner_outside_the_arc() {
    let near = BIG.radius * 0.05;
    assert!(!BIG.contains(near, BIG.height - near));
    assert!(!BIG.contains(BIG.width - near, BIG.height - near));
}

#[test]
fn square_top_corner_is_inside() {
    assert!(BIG.contains(0.5, 0.5));
    assert!(BIG.contains(BIG.width - 0.5, 0.5));
    assert!(BIG.contains(0.0, 0.0));
    // The whole top band above the bottom-corner zone is a plain rectangle.
    assert!(BIG.contains(0.5, BIG.height - BIG.radius));
}

#[test]
fn bottom_corner_is_rounded() {
    assert!(!BIG.contains(0.5, BIG.height - 0.5));
    assert!(!BIG.contains(BIG.width - 0.5, BIG.height - 0.5));
    // On the diagonal 0.35 r in from both edges is inside the arc (distance 0.92 r from centre).
    let inside = BIG.radius * 0.35;
    assert!(BIG.contains(inside, BIG.height - inside));
}

#[test]
fn contains_accepts_edges_and_centre() {
    let (mid_x, mid_y) = (BIG.width / 2.0, BIG.height / 2.0);
    assert!(BIG.contains(mid_x, mid_y));
    assert!(BIG.contains(0.0, mid_y));
    assert!(BIG.contains(BIG.width, mid_y));
    assert!(BIG.contains(mid_x, 0.0));
    assert!(BIG.contains(mid_x, BIG.height));
    assert!(BIG.contains(BIG.radius, 0.0));
}

#[test]
fn area_is_width_times_height() {
    assert!((BIG.area() - BIG.width * BIG.height).abs() < 1e-9);
}

fn centred_inside(outer: IslandShape, inner: IslandShape, x: f64, y: f64) -> bool {
    let inset = (outer.width - inner.width) / 2.0;
    outer.contains(x + inset, y)
}

/// Boundary samples of `shape` (relative to its own top-left): the four edges and the four arcs.
fn boundary(shape: IslandShape) -> Vec<(f64, f64)> {
    let r = shape.radius.min(shape.width / 2.0).min(shape.height / 2.0);
    let (w, h) = (shape.width, shape.height);
    let steps = 64;
    let mut points = Vec::new();
    for i in 0..=steps {
        let t = f64::from(i) / f64::from(steps);
        points.push((r + t * (w - 2.0 * r), 0.0));
        points.push((r + t * (w - 2.0 * r), h));
        points.push((0.0, r + t * (h - 2.0 * r)));
        points.push((w, r + t * (h - 2.0 * r)));
        let angle = t * std::f64::consts::FRAC_PI_2;
        let (dx, dy) = (r * angle.cos(), r * angle.sin());
        points.push((0.0, 0.0));
        points.push((w, 0.0));
        points.push((r - dx, h - r + dy));
        points.push((w - r + dx, h - r + dy));
    }
    // Nudge every sample a hair toward the centre so float noise on a shared edge is not a miss.
    let (cx, cy) = (w / 2.0, h / 2.0);
    points
        .into_iter()
        .map(|(x, y)| (cx + (x - cx) * (1.0 - 1e-9), cy + (y - cy) * (1.0 - 1e-9)))
        .collect()
}

fn assert_covers_both(a: IslandShape, b: IslandShape) {
    let both = cover(a, b);
    for shape in [a, b] {
        for (x, y) in boundary(shape) {
            assert!(
                centred_inside(both, shape, x, y),
                "{shape:?} sample ({x}, {y}) lies outside the cover {both:?}"
            );
        }
    }
}

#[test]
fn cover_contains_both_shapes() {
    let wide = IslandShape {
        width: 620.0,
        height: 100.0,
        radius: 30.0,
    };
    let tall = IslandShape {
        width: 300.0,
        height: 320.0,
        radius: 40.0,
    };
    assert_covers_both(wide, tall);
    assert_covers_both(tall, wide);
    assert_covers_both(REST, PAGES[0]);
    assert_covers_both(PAGES[0], REST);
    assert_covers_both(PAGES[1], PAGES[2]);
    // A mid-collapse pair: both sizes between REST and the page, radii still large.
    let mid = IslandShape {
        width: 400.0,
        height: 120.0,
        radius: 40.0,
    };
    assert_covers_both(PAGES[0], mid);
}

#[test]
fn cover_of_equal_shapes_is_the_shape() {
    assert_eq!(cover(BIG, BIG), BIG);
    assert_eq!(cover(REST, REST), REST);
}

#[test]
fn cover_uses_the_smaller_drawn_radius() {
    // REST draws radius 16 (half its height), not the 40 of the page.
    let both = cover(REST, BIG);
    assert!((both.radius - 16.0).abs() < 1e-9);
    assert!((both.width - BIG.width).abs() < 1e-9);
    assert!((both.height - BIG.height).abs() < 1e-9);
    // A radius that clamps to half the width counts as drawn, not as declared.
    let thin = IslandShape {
        width: 10.0,
        height: 50.0,
        radius: 40.0,
    };
    assert!((cover(thin, BIG).radius - 5.0).abs() < 1e-9);
}
