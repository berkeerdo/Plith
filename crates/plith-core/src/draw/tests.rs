use super::*;

#[test]
fn style_table_matches_the_design() {
    let title = style_spec(TextStyle::Title);
    assert_eq!(
        (title.family, title.size_dip, title.weight),
        (Family::Sans, 17.0, 600)
    );
    let name = style_spec(TextStyle::Name);
    assert_eq!(
        (name.family, name.size_dip, name.weight),
        (Family::Sans, 14.0, 400)
    );
    let sub = style_spec(TextStyle::Sub);
    assert_eq!(
        (sub.family, sub.size_dip, sub.uppercase),
        (Family::Mono, 10.0, true)
    );
    assert!(sub.tracking_em.abs() < f32::EPSILON);
    let label = style_spec(TextStyle::Label);
    assert!(label.uppercase && (label.tracking_em - 0.08).abs() < f32::EPSILON);
    let value = style_spec(TextStyle::Value);
    assert_eq!(
        (value.family, value.size_dip, value.uppercase),
        (Family::Mono, 12.0, false)
    );
    let clock = style_spec(TextStyle::Clock);
    assert_eq!(
        (clock.family, clock.size_dip, clock.weight),
        (Family::Mono, 14.0, 500)
    );
}
