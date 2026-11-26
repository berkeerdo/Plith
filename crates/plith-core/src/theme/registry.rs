//! The registry of theme packs and the resolve from a pack plus the user's picks to the palettes
//! the app draws with.

use std::sync::OnceLock;

use super::Palette;
use super::builtin::BUILTIN;
use super::colour::{Oklch, Rgb};
use super::pack::{Face, Floor, FlowStyle, InkSet, Pack, parse_pack};
use super::solve::{PlainInk, Shift, derive_plain, solve_accent};

/// Accents with less chroma than this are neutral: a grey is not a colour pick.
const NEUTRAL_CHROMA: f64 = 0.03;

/// The id of a theme pack: `[a-z0-9-]{1,32}`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ThemeId(String);

impl ThemeId {
    /// The id for a text, or `None` when it is not `[a-z0-9-]{1,32}`.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let ok = (1..=32).contains(&text.len())
            && text
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        ok.then(|| Self(text.to_owned()))
    }

    /// The id as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The id of the default pack, Gece.
    #[must_use]
    pub fn gece() -> Self {
        Self("gece".to_owned())
    }
}

impl Default for ThemeId {
    fn default() -> Self {
        Self::gece()
    }
}

/// The accent the user picked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccentPick {
    /// A chosen colour.
    Colour(Rgb),
    /// Follow the Windows accent colour.
    Windows,
}

/// The user's picks for one resolve.
#[derive(Debug, Clone, Copy)]
pub struct Picks<'a> {
    /// The accent pick, if any.
    pub accent: Option<&'a AccentPick>,
    /// The floor pick, if any.
    pub floor: Option<Rgb>,
    /// The current Windows accent colour, for [`AccentPick::Windows`].
    pub windows_accent: Option<Rgb>,
}

/// The palettes the app draws with.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedTheme {
    /// The id asked for, even when it is unknown and Gece is drawn.
    pub id: ThemeId,
    /// The pack actually drawn: `gece` for an unknown id.
    pub drawn: ThemeId,
    /// Whether the drawn open floor is a raster material (a plain floor pick replaces it).
    pub material: bool,
    /// The open island.
    pub open: Palette,
    /// The collapsed pill.
    pub collapsed: Palette,
    /// The sheet station: always on Gece's floor.
    pub sheet: Palette,
    /// Whether the open floor is dark.
    pub dark: bool,
    /// How the solve moved the accent on the open face.
    pub accent_shift: Shift,
    /// Whether a floor pick was refused for lack of readable ink.
    pub floor_refused: bool,
}

/// Every known theme pack.
#[derive(Debug, Clone)]
pub struct Registry {
    packs: Vec<Pack>,
}

/// Gece, parsed once: the fallback pack and the base of the sheet station.
fn gece_pack() -> &'static Pack {
    static GECE: OnceLock<Pack> = OnceLock::new();
    GECE.get_or_init(|| {
        parse_pack(BUILTIN[0].manifest)
            .unwrap_or_else(|e| panic!("built-in pack gece refused: {e}"))
    })
}

impl Registry {
    /// The built-in packs. A built-in that refuses to parse is a bug, so this panics with the
    /// refusal.
    ///
    /// # Panics
    /// When a built-in manifest is refused.
    #[must_use]
    pub fn builtin() -> Self {
        let packs = BUILTIN
            .iter()
            .map(|b| {
                parse_pack(b.manifest).unwrap_or_else(|e| panic!("built-in pack refused: {e}"))
            })
            .collect();
        Self { packs }
    }

    /// A registry of the given packs.
    #[must_use]
    pub fn from_packs(packs: Vec<Pack>) -> Self {
        Self { packs }
    }

    /// Every pack, in order.
    #[must_use]
    pub fn packs(&self) -> &[Pack] {
        &self.packs
    }

    /// The pack with an id.
    #[must_use]
    pub fn get(&self, id: &ThemeId) -> Option<&Pack> {
        self.packs.iter().find(|p| p.id == id.as_str())
    }

    /// The palettes of a theme under the user's picks. An unknown id draws Gece but keeps the id.
    #[must_use]
    pub fn resolve(&self, id: &ThemeId, picks: Picks) -> ResolvedTheme {
        let pack = self
            .get(id)
            .or_else(|| self.get(&ThemeId::gece()))
            .unwrap_or(gece_pack());
        let (plain, floor_refused) = plain_floor(pack.floor_adjustable, picks.floor);
        let (open, collapsed) = match &plain {
            Some(p) => (plain_face(p), plain_face(p)),
            None => (pack.open, pack.collapsed),
        };
        let wanted = wanted_accent(pack, &picks);
        let (open_flow, shift) = solve(pack, wanted, &open);
        let collapsed_flow = pack
            .collapsed_flow
            .unwrap_or_else(|| solve(pack, wanted, &collapsed).0);
        let sheet = sheet_palette(wanted);
        ResolvedTheme {
            id: id.clone(),
            drawn: ThemeId(pack.id.clone()),
            material: plain.is_none() && matches!(pack.open.floor, Floor::Material { .. }),
            open: palette(&open, open_flow),
            collapsed: palette(&collapsed, collapsed_flow),
            sheet,
            dark: plain.map_or(pack.dark, |p| p.dark),
            accent_shift: shift,
            floor_refused,
        }
    }
}

/// The derived plain floor of an adjustable pack, and whether the pick was refused.
fn plain_floor(adjustable: bool, pick: Option<Rgb>) -> (Option<PlainInk>, bool) {
    match pick {
        Some(floor) if adjustable => {
            let plain = derive_plain(floor);
            let refused = plain.is_none();
            (plain, refused)
        }
        _ => (None, false),
    }
}

fn plain_face(p: &PlainInk) -> Face {
    Face {
        floor: Floor::Colour {
            top: p.floor,
            high: p.floor_high,
        },
        ink: InkSet {
            ink: p.ink,
            muted: p.muted,
            faint: p.faint,
            track: p.track,
            faint_track: p.faint_track,
            border: p.border,
        },
    }
}

/// The accent before the solve: the pick, or the pack's own flow.
fn wanted_accent(pack: &Pack, picks: &Picks) -> Rgb {
    if !pack.flow_adjustable {
        return pack.flow;
    }
    match picks.accent {
        Some(AccentPick::Colour(c)) => *c,
        Some(AccentPick::Windows) => picks
            .windows_accent
            .filter(|c| Oklch::from_rgb(*c).c >= NEUTRAL_CHROMA)
            .unwrap_or(pack.flow),
        None => pack.flow,
    }
}

/// The accent solved against a face. A punch pack draws holes, so its accent is not solved.
fn solve(pack: &Pack, accent: Rgb, face: &Face) -> (Rgb, Shift) {
    if pack.flow_style == FlowStyle::Punch {
        return (accent, Shift::Kept);
    }
    let solved = solve_accent(accent, &pack.backdrop(face), pack.flow);
    (solved.colour, solved.shift)
}

/// Gece's open palette with the accent solved against Gece's floors, or Gece's lime when the
/// accent is neutral (a black accent would vanish on the black sheet station).
fn sheet_palette(accent: Rgb) -> Palette {
    let gece = gece_pack();
    let flow = if Oklch::from_rgb(accent).c < NEUTRAL_CHROMA {
        gece.flow
    } else {
        solve_accent(accent, &gece.backdrop(&gece.open), gece.flow).colour
    };
    palette(&gece.open, flow)
}

fn palette(face: &Face, flow: Rgb) -> Palette {
    let (floor, floor_high) = match face.floor {
        Floor::Colour { top, high } => (top.rgba(), high.rgba()),
        Floor::Material { mean, .. } => (mean.rgba(), mean.rgba()),
    };
    Palette {
        floor,
        floor_high,
        border: face.ink.border.rgba(),
        ink: face.ink.ink.rgba(),
        muted: face.ink.muted.rgba(),
        faint: face.ink.faint.rgba(),
        track: face.ink.track.rgba(),
        faint_track: face.ink.faint_track.rgba(),
        flow: flow.rgba(),
    }
}

#[cfg(test)]
mod tests;
