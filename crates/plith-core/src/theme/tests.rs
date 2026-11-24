use super::*;

/// A colour's channels as bits, for exact comparisons.
fn bits(c: [f32; 4]) -> [u32; 4] {
    c.map(f32::to_bits)
}

const THEMES: [Theme; 4] = [Theme::Gece, Theme::Kagit, Theme::York, Theme::Sodyum];

#[test]
fn every_theme_passes_contrast() {
    for theme in THEMES {
        let p = theme.palette();
        for (floor_name, floor) in [("floor", p.floor), ("floor_high", p.floor_high)] {
            for (name, ink, bar) in [
                ("ink", p.ink, 4.5),
                ("muted", p.muted, 4.5),
                ("faint", p.faint, 4.5),
                ("flow", p.flow, 3.0),
            ] {
                let ratio = contrast(ink, floor);
                assert!(
                    ratio >= bar,
                    "{theme:?} {name} on {floor_name}: {ratio:.2} < {bar}"
                );
            }
        }
    }
}

#[test]
fn contrast_matches_wcag_extremes() {
    let black = [0.0, 0.0, 0.0, 1.0];
    let white = [1.0, 1.0, 1.0, 1.0];
    assert!((contrast(black, white) - 21.0).abs() < 1e-6);
    assert!((contrast(white, black) - 21.0).abs() < 1e-6);
    assert!((contrast(white, white) - 1.0).abs() < 1e-9);
}

#[test]
fn gece_is_the_default_and_matches_the_spec() {
    assert_eq!(Theme::default(), Theme::Gece);
    let p = Palette::default();
    assert_eq!(p, Theme::Gece.palette());
    assert_eq!(bits(p.floor), bits(hex(0x0B, 0x0B, 0x0A)));
    assert_eq!(bits(p.floor_high), bits(hex(0x15, 0x14, 0x12)));
    assert_eq!(bits(p.flow), bits(hex(0xCA, 0xFF, 0x33)));
    assert_eq!(bits(p.faint), bits(hex(0x8A, 0x84, 0x7A)));
}

#[test]
fn faint_is_the_raised_value_per_theme() {
    assert_eq!(
        bits(Theme::Kagit.palette().faint),
        bits(hex(0x6B, 0x66, 0x5E))
    );
    assert_eq!(
        bits(Theme::York.palette().faint),
        bits(hex(0x86, 0x95, 0x8E))
    );
    assert_eq!(
        bits(Theme::Sodyum.palette().faint),
        bits(hex(0x86, 0x86, 0x8D))
    );
}

#[test]
fn resolve_maps_every_role() {
    let p = Theme::York.palette();
    assert_eq!(bits(p.resolve(Ink::Floor)), bits(p.floor));
    assert_eq!(bits(p.resolve(Ink::FloorHigh)), bits(p.floor_high));
    assert_eq!(bits(p.resolve(Ink::Border)), bits(p.border));
    assert_eq!(bits(p.resolve(Ink::Ink)), bits(p.ink));
    assert_eq!(bits(p.resolve(Ink::Muted)), bits(p.muted));
    assert_eq!(bits(p.resolve(Ink::Faint)), bits(p.faint));
    assert_eq!(bits(p.resolve(Ink::Track)), bits(p.track));
    assert_eq!(bits(p.resolve(Ink::FaintTrack)), bits(p.faint_track));
    assert_eq!(bits(p.resolve(Ink::Flow)), bits(p.flow));
}

#[test]
fn hex_channels_are_straight_rgba() {
    assert_eq!(bits(hex(0xFF, 0x00, 0x33)), bits([1.0, 0.0, 0.2, 1.0]));
}

#[test]
fn only_kagit_is_a_light_theme() {
    assert!(Theme::Gece.is_dark());
    assert!(Theme::York.is_dark());
    assert!(Theme::Sodyum.is_dark());
    assert!(!Theme::Kagit.is_dark());
    // The flag agrees with the floor: a dark theme has a floor darker than its ink.
    for theme in THEMES {
        let p = theme.palette();
        assert_eq!(
            theme.is_dark(),
            contrast(p.floor, [0.0, 0.0, 0.0, 1.0]) < 3.0,
            "{theme:?}"
        );
    }
}

#[test]
fn theme_keys_round_trip() {
    assert_eq!(Theme::ALL, THEMES);
    for theme in THEMES {
        assert_eq!(Theme::from_key(theme.key()), Some(theme));
    }
    assert_eq!(Theme::Gece.key(), "gece");
    assert_eq!(Theme::Kagit.key(), "kagit");
    assert_eq!(Theme::York.key(), "york");
    assert_eq!(Theme::Sodyum.key(), "sodyum");
    assert_eq!(Theme::from_key("neon"), None);
    assert_eq!(Theme::from_key("Gece"), None);
}
