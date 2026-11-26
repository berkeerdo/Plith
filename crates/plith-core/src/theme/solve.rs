//! The colour solve of theme packs: an accent pick moved to the nearest colour that is readable
//! on its backdrop, and the plain ink roles derived from a floor colour.

use super::colour::{Backdrop, Oklch, Rgb, worst_contrast};

/// The lightness step of every walk.
const STEP: f64 = 0.005;
/// The most steps a walk can take: lightness spans 0..=1.
const MAX_STEPS: u32 = 200;
/// The contrast an accent must reach against its backdrop.
const ACCENT_BAR: f64 = 3.0;
/// Floors with a luminance below this are dark.
const DARK_LUMINANCE: f64 = 0.179;
/// Chroma of the ink tints.
const INK_CHROMA: f64 = 0.012;
/// Floors with a chroma below this count as neutral and get untinted ink.
const NEUTRAL_CHROMA: f64 = 0.02;

/// How the solve moved an accent pick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shift {
    /// The pick passed and was kept.
    Kept,
    /// The pick was made lighter.
    Lighter,
    /// The pick was made darker.
    Darker,
    /// Nothing passed and the fallback colour was used.
    Fallback,
}

/// An accent after the solve.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Solved {
    /// The resulting colour.
    pub colour: Rgb,
    /// How it was reached.
    pub shift: Shift,
}

/// The lightness offsets of a walk outwards from the start, nearest first. At equal distance the
/// `first` direction (`1.0` or `-1.0`) comes before the other.
fn offsets(first: f64) -> impl Iterator<Item = f64> {
    (0..=MAX_STEPS).flat_map(move |i| {
        let d = f64::from(i) * STEP;
        if i == 0 {
            vec![0.0]
        } else {
            vec![first * d, -first * d]
        }
    })
}

/// The nearest passing colour on a lightness walk, with hue and chroma fixed.
fn walk(
    start: Oklch,
    chroma: f64,
    first: f64,
    bar: f64,
    backdrop: &Backdrop,
) -> Option<(Rgb, f64)> {
    offsets(first).find_map(|offset| {
        let l = start.l + offset;
        if !(0.0..=1.0).contains(&l) {
            return None;
        }
        let colour = Oklch {
            l,
            c: chroma,
            h: start.h,
        }
        .to_rgb();
        (worst_contrast(colour, backdrop) >= bar).then_some((colour, offset))
    })
}

/// Moves `pick` to the nearest lightness that reaches 3.0 contrast over `backdrop`, keeping its
/// hue and chroma (lighter wins a tie). Without a pass at that chroma it retries greyscale, and
/// then returns `fallback`.
#[must_use]
pub fn solve_accent(pick: Rgb, backdrop: &Backdrop, fallback: Rgb) -> Solved {
    if worst_contrast(pick, backdrop) >= ACCENT_BAR {
        return Solved {
            colour: pick,
            shift: Shift::Kept,
        };
    }
    let start = Oklch::from_rgb(pick);
    for chroma in [start.c, 0.0] {
        if let Some((colour, _)) = walk(start, chroma, 1.0, ACCENT_BAR, backdrop) {
            let shift = if colour.luminance() > pick.luminance() {
                Shift::Lighter
            } else {
                Shift::Darker
            };
            return Solved { colour, shift };
        }
    }
    Solved {
        colour: fallback,
        shift: Shift::Fallback,
    }
}

/// The colours of a plain theme derived from one floor colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlainInk {
    /// The floor gradient's top stop.
    pub floor: Rgb,
    /// The floor gradient's bottom stop.
    pub floor_high: Rgb,
    /// Primary text, contrast 7.0.
    pub ink: Rgb,
    /// Secondary text, contrast 4.5.
    pub muted: Rgb,
    /// Tertiary text, contrast 3.0.
    pub faint: Rgb,
    /// Slider, meter and route tracks.
    pub track: Rgb,
    /// The quiet track under traces and the tab line.
    pub faint_track: Rgb,
    /// Hairline borders.
    pub border: Rgb,
    /// Whether the floor is dark.
    pub dark: bool,
}

/// Derives the plain palette of a floor, or `None` when a text role cannot reach its bar (a mid
/// grey floor has no readable ink).
#[must_use]
pub fn derive_plain(floor: Rgb) -> Option<PlainInk> {
    let base = Oklch::from_rgb(floor);
    let dark = floor.luminance() < DARK_LUMINANCE;
    let sign = if dark { 1.0 } else { -1.0 };
    let shifted = |delta: f64| {
        Oklch {
            l: (base.l + sign * delta).clamp(0.0, 1.0),
            ..base
        }
        .to_rgb()
    };
    let floor_high = shifted(0.03);
    let backdrop = Backdrop::Flat {
        lums: vec![floor.luminance(), floor_high.luminance()],
    };
    // Targets per role; the walk prefers the side that raises contrast.
    let (targets, first) = if dark {
        ([0.94, 0.72, 0.62], 1.0)
    } else {
        ([0.20, 0.42, 0.52], -1.0)
    };
    let bars = [7.0, 4.5, 3.0];
    // A floor without a real hue has no tint to follow: keep the ink neutral.
    let ink_chroma = if base.c < NEUTRAL_CHROMA {
        0.0
    } else {
        INK_CHROMA
    };
    let mut roles = [floor; 3];
    for ((role, target), bar) in roles.iter_mut().zip(targets).zip(bars) {
        let start = Oklch {
            l: target,
            c: ink_chroma,
            h: base.h,
        };
        *role = walk(start, ink_chroma, first, bar, &backdrop)?.0;
    }
    let [ink, muted, faint] = roles;
    Some(PlainInk {
        floor,
        floor_high,
        ink,
        muted,
        faint,
        track: shifted(0.20),
        faint_track: shifted(0.09),
        border: shifted(0.12),
        dark,
    })
}

#[cfg(test)]
mod tests;
