use super::*;

fn frame(level: f32, muted: bool) -> OsdFrame {
    OsdFrame {
        opacity: 1.0,
        level,
        meter: Meter::Volume,
        muted,
        label: String::from("Speakers"),
    }
}

#[test]
fn value_is_a_whole_percent_or_the_mute_word() {
    assert_eq!(value_text(&frame(0.424, false)), "42");
    assert_eq!(value_text(&frame(1.0, false)), "100");
    assert_eq!(value_text(&frame(0.0, false)), "0");
    assert_eq!(value_text(&frame(0.5, true)), MUTED_WORD);
}

#[test]
fn ticks_span_the_ruler_every_ten_percent() {
    assert!((tick_x(0) - RULER.0).abs() < f32::EPSILON);
    assert!((tick_x(10) - RULER.1).abs() < 1e-4);
    assert!((tick_x(5) - fill_right(0.5)).abs() < 1e-4);
}

#[test]
fn fill_is_clamped_to_the_ruler() {
    assert!((fill_right(-1.0) - RULER.0).abs() < f32::EPSILON);
    assert!((fill_right(2.0) - RULER.1).abs() < 1e-4);
}

#[test]
fn slots_do_not_overlap() {
    assert!(PAD < LABEL_RIGHT && LABEL_RIGHT < RULER.0);
    assert!(RULER.1 < VALUE_LEFT && VALUE_LEFT < WIDTH - PAD);
}

#[test]
fn family_falls_back_to_the_last_choice() {
    assert_eq!(pick(|name| name == "Martian Mono"), "Martian Mono");
    assert_eq!(pick(|name| name == "Consolas"), "Consolas");
    assert_eq!(pick(|_| false), "Consolas");
}

fn pick(installed: impl Fn(&str) -> bool) -> &'static str {
    text::pick_family(&text::MONO_FAMILIES, installed)
}

#[test]
fn snap_lands_on_device_pixels() {
    for scale in [1.0_f32, 1.25, 1.5] {
        for dip in [0.0_f32, 19.5, 20.0, 20.4, 92.0, 236.0] {
            let px = snap(dip, scale) * scale;
            assert!((px - px.round()).abs() < 1e-4, "{dip} at {scale}: {px}");
            assert!((snap(dip, scale) - dip).abs() <= 0.5 / scale + 1e-4);
        }
    }
    assert!((snap(19.5, 1.0) - 20.0).abs() < 1e-6);
    assert!((snap(19.5, 1.25) - 19.2).abs() < 1e-4);
    assert!((snap(19.5, 1.5) - 19.333_334).abs() < 1e-4);
}

#[test]
fn snapped_span_is_at_least_one_device_pixel() {
    for scale in [1.0_f32, 1.25, 1.5] {
        let (a, b) = snap_span(19.5, 1.0, scale);
        let px = (b - a) * scale;
        assert!(px >= 1.0 - 1e-4, "1 DIP at {scale} is {px} px");
        assert!((px - px.round()).abs() < 1e-4);
        let (a, b) = snap_span(20.0, 0.1, scale);
        assert!(
            ((b - a) * scale - 1.0).abs() < 1e-4,
            "a hairline keeps one pixel"
        );
    }
}

#[test]
fn labels_upper_case_the_turkish_way() {
    assert_eq!(upper_tr("Mikrofon"), "MİKROFON");
    assert_eq!(upper_tr("Kulaklık"), "KULAKLIK");
    assert_eq!(upper_tr("Speakers"), "SPEAKERS");
    assert_eq!(upper_tr("şğüöç"), "ŞĞÜÖÇ");
}
