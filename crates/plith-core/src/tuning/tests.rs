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

#[test]
fn brisk_zetas_match_the_plan() {
    assert!(
        (BRISK.grow_h.zeta() - 0.81).abs() < 0.006,
        "{}",
        BRISK.grow_h.zeta()
    );
    assert!((BRISK.card_grow_w.zeta() - 0.89).abs() < 0.006);
    assert!((BRISK.card_grow_h.zeta() - 0.89).abs() < 0.006);
}

#[test]
fn brisk_differs_from_r1_only_in_the_planned_fields() {
    assert_eq!(BRISK.grow_h, Tuning::new(320.0, 29.0));
    assert_eq!(BRISK.card_grow_w, Tuning::new(500.0, 40.0));
    assert_eq!(BRISK.card_grow_h, Tuning::new(500.0, 40.0));
    let same = TuningSet {
        grow_h: R1.grow_h,
        card_grow_w: R1.card_grow_w,
        card_grow_h: R1.card_grow_h,
        name: R1.name,
        ..BRISK
    };
    assert_eq!(same, R1);
    assert_eq!((R1.card_grow_w, R1.card_grow_h), (GROW_W, GROW_H));
}

#[test]
fn presets_are_found_by_name() {
    assert_eq!(TuningSet::by_name("r1"), Some(R1));
    assert_eq!(TuningSet::by_name("brisk"), Some(BRISK));
    assert_eq!(TuningSet::by_name("fast"), None);
    assert_eq!(TuningSet::default(), R1);
}
