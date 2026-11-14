use super::*;

#[test]
fn zetas_match_the_table() {
    for (t, z) in [
        (GROW_W, 0.73),
        (GROW_H, 0.73),
        (SHRINK_W, 0.90),
        (SHRINK_H, 0.91),
        (RADIUS_GROW, 1.0),
        (RADIUS_SHRINK, 1.0),
        (PAGE_OFFSET, 0.90),
        (OPACITY, 1.0),
    ] {
        assert!((t.zeta() - z).abs() < 0.01, "{t:?} zeta {}", t.zeta());
    }
}
