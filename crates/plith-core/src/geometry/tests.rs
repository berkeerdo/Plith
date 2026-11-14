use super::*;

const BIG: IslandShape = IslandShape {
    width: 620.0,
    height: 216.0,
    radius: 40.0,
};

#[test]
fn contains_rejects_the_corner_outside_the_arc() {
    assert!(!BIG.contains(1.0, 1.0));
    assert!(!BIG.contains(619.0, 215.0));
}

#[test]
fn contains_accepts_edges_and_centre() {
    assert!(BIG.contains(310.0, 108.0));
    assert!(BIG.contains(0.0, 108.0));
    assert!(BIG.contains(620.0, 108.0));
    assert!(BIG.contains(310.0, 0.0));
    assert!(BIG.contains(310.0, 216.0));
    assert!(BIG.contains(40.0, 0.0));
}

#[test]
fn area_is_width_times_height() {
    assert!((BIG.area() - 620.0 * 216.0).abs() < 1e-9);
}
