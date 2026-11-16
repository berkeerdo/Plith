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
