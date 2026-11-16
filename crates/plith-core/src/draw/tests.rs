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

fn timeline(playing: bool) -> SeekTimeline {
    SeekTimeline {
        position_s: 30.0,
        duration_s: 120.0,
        stamp: SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1000),
        playing,
    }
}

#[test]
fn seek_fraction_extrapolates_while_playing() {
    let t = timeline(true);
    let at = |s: u64| t.stamp + std::time::Duration::from_secs(s);
    assert!((t.fraction_at(at(0)) - 0.25).abs() < 1e-9);
    assert!((t.fraction_at(at(30)) - 0.5).abs() < 1e-9);
    assert!(
        (t.fraction_at(at(500)) - 1.0).abs() < 1e-9,
        "clamped to the end"
    );
    // A clock before the stamp counts as no time elapsed.
    assert!((t.fraction_at(SystemTime::UNIX_EPOCH) - 0.25).abs() < 1e-9);
}

#[test]
fn seek_fraction_holds_while_paused_and_guards_zero_length() {
    let t = timeline(false);
    assert!((t.fraction_at(t.stamp + std::time::Duration::from_secs(30)) - 0.25).abs() < 1e-9);
    let empty = SeekTimeline {
        duration_s: 0.0,
        ..timeline(true)
    };
    assert!(empty.fraction_at(empty.stamp).abs() < 1e-9);
}
