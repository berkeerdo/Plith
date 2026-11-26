use super::*;

/// A colour's channels as bits, for exact comparisons.
fn bits(c: [f32; 4]) -> [u32; 4] {
    c.map(f32::to_bits)
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
fn resolve_maps_every_role() {
    let p = Palette::setup();
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
fn setup_palette_is_lime_with_readable_text() {
    let p = Palette::setup();
    assert_eq!(bits(p.floor), bits(hex(204, 251, 12)));
    for (name, ink) in [("ink", p.ink), ("muted", p.muted), ("faint", p.faint)] {
        assert!(contrast(ink, p.floor) >= 4.5, "{name} on the lime floor");
    }
    // The button label is lime on the black pill.
    assert!(contrast(p.flow, [0.0, 0.0, 0.0, 1.0]) >= 7.0);
}
