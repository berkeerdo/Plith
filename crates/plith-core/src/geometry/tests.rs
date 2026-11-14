use super::*;

use crate::shapes::PAGES;

const BIG: IslandShape = PAGES[0];

#[test]
fn contains_rejects_the_corner_outside_the_arc() {
    let near = BIG.radius * 0.05;
    assert!(!BIG.contains(near, near));
    assert!(!BIG.contains(BIG.width - near, BIG.height - near));
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
