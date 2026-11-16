//! Text system tests: DirectWrite only, no GPU, CI-safe.

use plith_core::draw::{Family, TextStyle, Trim};
use plith_render::TextSystem;

fn system() -> TextSystem {
    TextSystem::new().expect("text system")
}

#[test]
fn bundled_fonts_load_under_the_expected_names() {
    let mut text = system();
    assert!(text.uses_bundled_fonts(), "notes: {:?}", text.take_notes());
    assert_eq!(text.family_name(Family::Sans), "Public Sans");
    assert_eq!(text.family_name(Family::Mono), "Martian Mono");
    assert!(text.take_notes().is_empty());
    // A layout in each family resolves without error.
    text.layout("Aa", TextStyle::Title, 200.0, Trim::End)
        .unwrap();
    text.layout("Aa", TextStyle::Clock, 200.0, Trim::End)
        .unwrap();
}

#[test]
fn mono_digits_share_one_advance() {
    let mut text = system();
    for style in [TextStyle::Value, TextStyle::Clock] {
        let ones = text.measure("1111", style, 400.0, Trim::End).unwrap();
        let eights = text.measure("8888", style, 400.0, Trim::End).unwrap();
        assert!(ones > 0.0);
        assert!(
            (ones - eights).abs() < 0.01,
            "{style:?}: {ones} vs {eights}"
        );
    }
}

#[test]
fn middle_trim_keeps_both_ends() {
    let mut text = system();
    let full = "Speakers (Realtek(R) High Definition Audio Device)";
    let shown = text
        .display_text(full, TextStyle::Name, 120.0, Trim::Middle)
        .unwrap();
    assert!(shown.starts_with("Spea"), "{shown}");
    assert!(shown.ends_with("ice)"), "{shown}");
    assert!(shown.contains('\u{2026}'), "{shown}");
    let width = text
        .measure(full, TextStyle::Name, 120.0, Trim::Middle)
        .unwrap();
    assert!(width <= 120.0, "{width}");
    // Wide enough: unchanged.
    let whole = text
        .display_text(full, TextStyle::Name, 2000.0, Trim::Middle)
        .unwrap();
    assert_eq!(whole, full);
}

#[test]
fn end_trim_layout_is_built_and_clipped() {
    let mut text = system();
    let full = "Speakers (Realtek(R) High Definition Audio Device)";
    let layout = text
        .layout(full, TextStyle::Name, 120.0, Trim::End)
        .unwrap();
    let mut metrics = windows::Win32::Graphics::DirectWrite::DWRITE_TEXT_METRICS::default();
    // SAFETY: the out pointer refers to a live local.
    unsafe { layout.GetMetrics(&raw mut metrics).unwrap() };
    assert_eq!(metrics.lineCount, 1);
}

#[test]
fn label_style_is_upper_cased_and_tracked() {
    let mut text = system();
    assert_eq!(
        text.display_text("input", TextStyle::Label, 400.0, Trim::End)
            .unwrap(),
        "INPUT"
    );
    // Every mono role carries the mock's 0.06 em tracking, so Label and Sub (same size, same
    // tracking) measure the same.
    let label = text
        .measure("ABCD", TextStyle::Label, 400.0, Trim::End)
        .unwrap();
    let sub = text
        .measure("ABCD", TextStyle::Sub, 400.0, Trim::End)
        .unwrap();
    assert!((label - sub).abs() < 0.01, "{label} vs {sub}");
    assert!(plith_core::draw::style_spec(TextStyle::Label).tracking_em > 0.0);
}

#[test]
fn cache_reuses_layouts() {
    let mut text = system();
    text.layout("Mikrofon", TextStyle::Name, 200.0, Trim::End)
        .unwrap();
    assert_eq!((text.cache_len(), text.cache_hits()), (1, 0));
    text.layout("Mikrofon", TextStyle::Name, 200.0, Trim::End)
        .unwrap();
    assert_eq!((text.cache_len(), text.cache_hits()), (1, 1));
    text.layout("Mikrofon", TextStyle::Name, 150.0, Trim::End)
        .unwrap();
    assert_eq!(text.cache_len(), 2);
}

#[test]
fn cache_clears_when_full() {
    let mut text = system();
    for i in 0..600 {
        text.layout(&format!("line {i}"), TextStyle::Name, 200.0, Trim::End)
            .unwrap();
    }
    assert!(text.cache_len() <= 512);
    assert!(text.cache_len() > 0);
}

#[test]
fn missing_font_falls_back() {
    let mut text = TextSystem::new_system_fonts().expect("fallback system");
    assert!(!text.uses_bundled_fonts());
    let notes = text.take_notes();
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(["Segoe UI Variable Text", "Segoe UI"].contains(&text.family_name(Family::Sans)));
    assert!(["Cascadia Mono", "Consolas"].contains(&text.family_name(Family::Mono)));
    let width = text
        .measure("Fallback", TextStyle::Name, 400.0, Trim::End)
        .unwrap();
    assert!(width > 0.0);
    text.layout("12:34", TextStyle::Clock, 100.0, Trim::End)
        .unwrap();
}

#[test]
fn bundled_faces_resolve_to_the_exact_weights() {
    let text = system();
    // Sans: Regular 400 (Name) and SemiBold 600 (Title); Mono: Regular 400 (Value and Clock).
    // A missing face or the wrong family model would match the nearest weight.
    for (style, weight) in [
        (TextStyle::Name, 400),
        (TextStyle::Title, 600),
        (TextStyle::Value, 400),
        (TextStyle::Clock, 400),
    ] {
        assert_eq!(
            text.matched_weight(style).unwrap(),
            Some(weight),
            "{style:?}"
        );
    }
}
