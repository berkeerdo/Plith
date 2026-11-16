use super::*;

const SHAPE: IslandShape = IslandShape {
    width: 100.0,
    height: 60.0,
    radius: 20.0,
};

#[test]
fn outline_starts_top_left_clockwise() {
    let segs = island_outline(SHAPE, (10.0, 5.0));
    // First segment runs from the top-left origin to the top-right.
    assert_eq!(segs[0], PathSeg::Line(110.0, 5.0));
    // Then straight down to where the bottom-right arc starts.
    assert_eq!(segs[1], PathSeg::Line(110.0, 45.0));
    assert_eq!(
        segs[2],
        PathSeg::Arc {
            to: (90.0, 65.0),
            radius: 20.0
        }
    );
    assert_eq!(segs[3], PathSeg::Line(30.0, 65.0));
    assert_eq!(
        segs[4],
        PathSeg::Arc {
            to: (10.0, 45.0),
            radius: 20.0
        }
    );
    // Closes back to the start.
    assert_eq!(segs[5], PathSeg::Line(10.0, 5.0));
    assert_eq!(segs.len(), 6);
}

#[test]
fn zero_radius_gives_only_lines() {
    let shape = IslandShape {
        radius: 0.0,
        ..SHAPE
    };
    let segs = island_outline(shape, (0.0, 0.0));
    assert_eq!(
        segs,
        vec![
            PathSeg::Line(100.0, 0.0),
            PathSeg::Line(100.0, 60.0),
            PathSeg::Line(0.0, 60.0),
            PathSeg::Line(0.0, 0.0),
        ]
    );
}

#[test]
fn radius_is_clamped_like_drawn_radius() {
    let shape = IslandShape {
        radius: 500.0,
        ..SHAPE
    };
    let segs = island_outline(shape, (0.0, 0.0));
    let radii: Vec<f64> = segs
        .iter()
        .filter_map(|s| match s {
            PathSeg::Arc { radius, .. } => Some(*radius),
            PathSeg::Line(..) => None,
        })
        .collect();
    assert_eq!(radii, vec![30.0, 30.0]);
}

#[test]
fn every_arc_endpoint_is_on_the_shape() {
    for shape in [crate::shapes::REST, crate::shapes::PAGES[0]] {
        for seg in island_outline(shape, (0.0, 0.0)) {
            if let PathSeg::Arc { to, .. } = seg {
                assert!(shape.contains(to.0, to.1), "{to:?}");
            }
        }
    }
}
