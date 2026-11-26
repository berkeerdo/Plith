#![allow(clippy::float_cmp)]

use super::*;
use crate::theme::colour::Oklch;
use crate::theme::fixtures::HALKA;
use crate::theme::{contrast, hex};

/// Today's Gece palette (the old `Theme::Gece` arm), copied so the test outlives the enum.
fn todays_gece() -> Palette {
    Palette {
        floor: hex(0x0B, 0x0B, 0x0A),
        floor_high: hex(0x15, 0x14, 0x12),
        border: hex(0x2B, 0x28, 0x24),
        ink: hex(0xEC, 0xE9, 0xE2),
        muted: hex(0xA3, 0x9E, 0x94),
        faint: hex(0x8A, 0x84, 0x7A),
        track: hex(0x4A, 0x46, 0x3F),
        faint_track: hex(0x2E, 0x2B, 0x27),
        flow: hex(0xCA, 0xFF, 0x33),
    }
}

fn rgb(text: &str) -> Rgb {
    Rgb::parse(text).unwrap()
}

fn picks<'a>(accent: Option<&'a AccentPick>, floor: Option<&str>) -> Picks<'a> {
    Picks {
        accent,
        floor: floor.map(rgb),
        windows_accent: None,
    }
}

fn none() -> Picks<'static> {
    picks(None, None)
}

fn gece() -> ThemeId {
    ThemeId::gece()
}

#[test]
fn gece_no_picks_is_todays_palette() {
    let r = Registry::builtin().resolve(&gece(), none());
    assert_eq!(r.open, todays_gece());
    assert_eq!(r.collapsed, todays_gece());
    assert_eq!(r.sheet, todays_gece());
    assert!(r.dark);
    assert_eq!(r.accent_shift, Shift::Kept);
    assert!(!r.floor_refused);
}

#[test]
fn unknown_id_draws_gece_keeps_id() {
    let id = ThemeId::parse("firn").unwrap();
    let r = Registry::builtin().resolve(&id, none());
    assert_eq!(r.id.as_str(), "firn");
    assert_eq!(r.open, todays_gece());
    assert_eq!(r.collapsed, todays_gece());
}

#[test]
fn id_parse_rules() {
    assert!(ThemeId::parse("").is_none());
    assert!(ThemeId::parse("Gece").is_none());
    assert!(ThemeId::parse(&"a".repeat(33)).is_none());
    assert!(ThemeId::parse("a-1").is_some());
    assert_eq!(ThemeId::default(), ThemeId::gece());
}

#[test]
fn sodyum_colours_through_gece() {
    let pick = AccentPick::Colour(rgb("#FFB224"));
    let r = Registry::builtin().resolve(&gece(), picks(Some(&pick), None));
    assert_eq!(r.open.flow, hex(0xFF, 0xB2, 0x24));
    assert_eq!(r.accent_shift, Shift::Kept);
}

#[test]
fn kagit_colours_through_gece() {
    let pick = AccentPick::Colour(rgb("#3F6E00"));
    let r = Registry::builtin().resolve(&gece(), picks(Some(&pick), Some("#F3F2ED")));
    assert_eq!(r.open.floor, hex(0xF3, 0xF2, 0xED));
    assert!(!r.dark);
    assert!(contrast(r.open.flow, r.open.floor) >= 3.0);
    assert!(contrast(r.open.flow, r.open.floor_high) >= 3.0);
    assert!(contrast(r.collapsed.flow, r.collapsed.floor) >= 3.0);
}

#[test]
fn york_colours_through_gece() {
    let pick = AccentPick::Colour(rgb("#FFE9A8"));
    let r = Registry::builtin().resolve(&gece(), picks(Some(&pick), Some("#1C2420")));
    assert_eq!(r.open.floor, hex(0x1C, 0x24, 0x20));
    assert!(r.dark);
    assert_eq!(r.open.flow, hex(0xFF, 0xE9, 0xA8));
}

fn to_rgb(c: [f32; 4]) -> Rgb {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // a channel in 0..=1
    let byte = |v: f32| (v * 255.0).round() as u8;
    Rgb([byte(c[0]), byte(c[1]), byte(c[2])])
}

#[test]
fn windows_accent_followed() {
    let pick = AccentPick::Windows;
    let blue = rgb("#0078D4");
    let r = Registry::builtin().resolve(
        &gece(),
        Picks {
            accent: Some(&pick),
            floor: None,
            windows_accent: Some(blue),
        },
    );
    assert!(contrast(r.open.flow, r.open.floor) >= 3.0);
    let got = Oklch::from_rgb(to_rgb(r.open.flow));
    let want = Oklch::from_rgb(blue);
    let mut dh = (got.h - want.h).abs();
    if dh > 180.0 {
        dh = 360.0 - dh;
    }
    assert!(dh < 10.0, "hue moved by {dh}");
}

#[test]
fn neutral_windows_accent_uses_pack_flow() {
    let pick = AccentPick::Windows;
    let r = Registry::builtin().resolve(
        &gece(),
        Picks {
            accent: Some(&pick),
            floor: None,
            windows_accent: Some(rgb("#808080")),
        },
    );
    assert_eq!(r.open.flow, todays_gece().flow);
}

#[test]
fn sheet_keeps_gece_floor() {
    let pick = AccentPick::Colour(rgb("#3F6E00"));
    let r = Registry::builtin().resolve(&gece(), picks(Some(&pick), Some("#F3F2ED")));
    assert_eq!(r.sheet.floor, hex(0x0B, 0x0B, 0x0A));
    assert!(contrast(r.sheet.flow, r.sheet.floor) >= 3.0);
}

fn halka_registry() -> Registry {
    let mut packs = Registry::builtin().packs().to_vec();
    packs.push(parse_pack(HALKA).unwrap());
    Registry::from_packs(packs)
}

#[test]
fn black_accent_sheet_station_is_lime() {
    let id = ThemeId::parse("halka").unwrap();
    let r = halka_registry().resolve(&id, none());
    assert_eq!(r.sheet.flow, todays_gece().flow);
    assert_eq!(r.open.flow, hex(0, 0, 0));
    assert!(contrast(r.collapsed.flow, r.collapsed.floor) >= 3.0);
}

#[test]
fn material_ignores_floor_pick() {
    let id = ThemeId::parse("halka").unwrap();
    let a = halka_registry().resolve(&id, none());
    let b = halka_registry().resolve(&id, picks(None, Some("#F3F2ED")));
    assert_eq!(a, b);
    assert!(!b.floor_refused);
}

#[test]
fn explicit_white_accent_kept() {
    let pick = AccentPick::Colour(rgb("#FFFFFF"));
    let r = Registry::builtin().resolve(&gece(), picks(Some(&pick), None));
    assert_eq!(r.open.flow, hex(0xFF, 0xFF, 0xFF));
    assert_eq!(r.accent_shift, Shift::Kept);
}

#[test]
fn explicit_grey_accent_solved_not_dropped() {
    let pick = AccentPick::Colour(rgb("#808080"));
    let r = Registry::builtin().resolve(&gece(), picks(Some(&pick), None));
    assert_ne!(r.open.flow, todays_gece().flow);
    assert!(contrast(r.open.flow, r.open.floor) >= 3.0);
    assert!(contrast(r.open.flow, r.open.floor_high) >= 3.0);
}

#[test]
fn bad_floor_sets_refused() {
    let r = Registry::builtin().resolve(&gece(), picks(None, Some("#777777")));
    assert!(r.floor_refused);
    assert_eq!(r.open, todays_gece());
}
