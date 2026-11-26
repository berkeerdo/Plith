#![allow(clippy::float_cmp)]

use super::*;

#[test]
fn parse_accepts_hex_only() {
    assert!(Rgb::parse("#CAFF33").is_some());
    assert!(Rgb::parse("#caff33").is_some());
    for bad in ["CAFF33", "#CAFF3", "#CAFF333", "#GGGGGG", ""] {
        assert!(Rgb::parse(bad).is_none(), "{bad}");
    }
}

#[test]
fn hex_round_trips() {
    assert_eq!(Rgb::parse("#caff33").unwrap().to_hex(), "#CAFF33");
}

#[test]
fn rgba_matches_hex_fn() {
    assert_eq!(Rgb([0xCA, 0xFF, 0x33]).rgba(), hex(0xCA, 0xFF, 0x33));
}

#[test]
fn oklch_round_trips_in_gamut() {
    for r in (0..=255u8).step_by(17) {
        for g in (0..=255u8).step_by(17) {
            for b in (0..=255u8).step_by(17) {
                let back = Oklch::from_rgb(Rgb([r, g, b])).to_rgb();
                for (i, want) in [r, g, b].into_iter().enumerate() {
                    let d = i16::from(back.0[i]) - i16::from(want);
                    assert!(d.abs() <= 1, "{r},{g},{b} -> {:?}", back.0);
                }
            }
        }
    }
}

#[test]
fn oklch_white_and_black() {
    let white = Oklch::from_rgb(Rgb([255, 255, 255]));
    let black = Oklch::from_rgb(Rgb([0, 0, 0]));
    assert!((white.l - 1.0).abs() < 0.001 && white.c < 0.001);
    assert!(black.l.abs() < 0.001 && black.c < 0.001);
}

#[test]
fn out_of_gamut_keeps_hue_and_lightness() {
    let back = Oklch::from_rgb(
        Oklch {
            l: 0.7,
            c: 0.4,
            h: 150.0,
        }
        .to_rgb(),
    );
    assert!((back.l - 0.7).abs() < 0.01, "l {}", back.l);
    assert!((back.h - 150.0).abs() < 2.0, "h {}", back.h);
}

#[test]
fn range_inside_is_one() {
    let range = Backdrop::Range { min: 0.1, max: 0.5 };
    assert!((worst_contrast(Rgb([128, 128, 128]), &range) - 1.0).abs() < f64::EPSILON);
}

#[test]
fn range_outside_uses_nearer_end() {
    let range = Backdrop::Range { min: 0.2, max: 0.9 };
    assert!((worst_contrast(Rgb([0, 0, 0]), &range) - 5.0).abs() < 1e-9);
}

#[test]
fn flat_takes_the_minimum() {
    let flat = Backdrop::Flat {
        lums: vec![0.0, 1.0],
    };
    let got = worst_contrast(Rgb([128, 128, 128]), &flat);
    let lum = Rgb([128, 128, 128]).luminance();
    let want = ratio(lum, 0.0).min(ratio(lum, 1.0));
    assert!((got - want).abs() < 1e-9);
    assert!(got < 5.0);
}
