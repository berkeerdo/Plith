//! The colour model of theme packs: 8-bit sRGB, OKLCH and contrast against a backdrop.

// The OKLab reference formulas use single-letter names (l, m, s, a, b).
#![allow(clippy::many_single_char_names)]

use super::{hex, luminance};

/// An opaque colour from 8-bit sRGB channels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Rgb(pub [u8; 3]);

impl Rgb {
    /// Parses `#` followed by exactly six hex digits, in either case.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let digits = text.strip_prefix('#')?;
        if digits.len() != 6 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let channel = |i: usize| u8::from_str_radix(&digits[i..i + 2], 16).ok();
        Some(Self([channel(0)?, channel(2)?, channel(4)?]))
    }

    /// The upper case `#RRGGBB` form.
    #[must_use]
    pub fn to_hex(self) -> String {
        let [r, g, b] = self.0;
        format!("#{r:02X}{g:02X}{b:02X}")
    }

    /// The colour as straight sRGB RGBA in 0..=1, the same as [`hex`].
    #[must_use]
    pub fn rgba(self) -> [f32; 4] {
        let [r, g, b] = self.0;
        hex(r, g, b)
    }

    /// WCAG relative luminance.
    #[must_use]
    pub fn luminance(self) -> f64 {
        luminance(self.rgba())
    }
}

/// A colour in OKLCH: lightness 0..=1, chroma, hue in degrees 0..360.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Oklch {
    /// Perceptual lightness.
    pub l: f64,
    /// Chroma.
    pub c: f64,
    /// Hue angle in degrees.
    pub h: f64,
}

fn to_linear(channel: u8) -> f64 {
    let c = f64::from(channel) / 255.0;
    if c <= 0.040_45 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn from_linear(value: f64) -> u8 {
    let v = value.clamp(0.0, 1.0);
    let c = if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    };
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let byte = (c * 255.0).round() as u8;
    byte
}

/// Linear sRGB channels of an OKLCH colour, unclamped.
fn linear_of(l: f64, c: f64, h: f64) -> [f64; 3] {
    let (sin, cos) = h.to_radians().sin_cos();
    let (a, b) = (c * cos, c * sin);
    let lp = l + 0.396_337_777_4 * a + 0.215_803_757_3 * b;
    let mp = l - 0.105_561_345_8 * a - 0.063_854_172_8 * b;
    let sp = l - 0.089_484_177_5 * a - 1.291_485_548_0 * b;
    let (lc, mc, sc) = (lp * lp * lp, mp * mp * mp, sp * sp * sp);
    [
        4.076_741_662_1 * lc - 3.307_711_591_3 * mc + 0.230_969_929_2 * sc,
        -1.268_438_004_6 * lc + 2.609_757_401_1 * mc - 0.341_319_396_5 * sc,
        -0.004_196_086_3 * lc - 0.703_418_614_7 * mc + 1.707_614_701_0 * sc,
    ]
}

fn in_gamut(rgb: [f64; 3]) -> bool {
    const EPS: f64 = 1e-6;
    rgb.iter().all(|v| (-EPS..=1.0 + EPS).contains(v))
}

impl Oklch {
    /// The OKLCH form of an sRGB colour.
    #[must_use]
    pub fn from_rgb(rgb: Rgb) -> Self {
        let [r, g, b] = rgb.0.map(to_linear);
        let l = (0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b).cbrt();
        let m = (0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b).cbrt();
        let s = (0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b).cbrt();
        let lab_l = 0.210_454_255_3 * l + 0.793_617_785_0 * m - 0.004_072_046_8 * s;
        let a = 1.977_998_495_1 * l - 2.428_592_205_0 * m + 0.450_593_709_9 * s;
        let b = 0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766_0 * s;
        Self {
            l: lab_l,
            c: a.hypot(b),
            h: b.atan2(a).to_degrees().rem_euclid(360.0),
        }
    }

    /// The nearest sRGB colour: lightness and hue are kept and chroma is reduced until the colour
    /// fits the gamut.
    #[must_use]
    pub fn to_rgb(self) -> Rgb {
        let l = self.l.clamp(0.0, 1.0);
        let direct = linear_of(l, self.c, self.h);
        let linear = if in_gamut(direct) {
            direct
        } else {
            let (mut lo, mut hi) = (0.0, self.c);
            for _ in 0..40 {
                let mid = f64::midpoint(lo, hi);
                if in_gamut(linear_of(l, mid, self.h)) {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            linear_of(l, lo, self.h)
        };
        Rgb(linear.map(from_linear))
    }
}

/// What a colour is drawn against.
#[derive(Debug, Clone, PartialEq)]
pub enum Backdrop {
    /// A set of flat backdrops, given as WCAG luminances.
    Flat {
        /// The luminance of each backdrop.
        lums: Vec<f64>,
    },
    /// Any backdrop whose luminance lies in `min..=max`.
    Range {
        /// The darkest luminance.
        min: f64,
        /// The lightest luminance.
        max: f64,
    },
}

fn ratio(a: f64, b: f64) -> f64 {
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// The lowest WCAG contrast ratio of a colour over the backdrop. Inside a range the worst case is
/// reported as `1.0`, since some backdrop there matches the colour.
#[must_use]
pub fn worst_contrast(colour: Rgb, backdrop: &Backdrop) -> f64 {
    let lum = colour.luminance();
    match backdrop {
        Backdrop::Flat { lums } => lums
            .iter()
            .map(|&l| ratio(lum, l))
            .fold(f64::INFINITY, f64::min),
        Backdrop::Range { min, max } => {
            if (*min..=*max).contains(&lum) {
                1.0
            } else if lum < *min {
                ratio(lum, *min)
            } else {
                ratio(lum, *max)
            }
        }
    }
}

#[cfg(test)]
mod tests;
