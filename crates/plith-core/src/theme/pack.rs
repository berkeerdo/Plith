//! A theme pack: the typed form of a `theme.ini` manifest (spec 4.2) and its validation. A pack
//! that fails any check is refused whole, with the key and a reason.

use std::fmt;

use super::colour::{Backdrop, Rgb, worst_contrast};
use parse::{Fields, LANGS};

mod parse;

/// The most swatches a pack may suggest.
const MAX_SWATCHES: usize = 6;
/// Floors with a luminance below this are dark.
const DARK_LUMINANCE: f64 = 0.179;
/// The contrast bars: primary ink, muted ink, faint ink and the flow accent.
const INK_BAR: f64 = 7.0;
const MUTED_BAR: f64 = 4.5;
const FAINT_BAR: f64 = 3.0;
const FLOW_BAR: f64 = 3.0;

/// Why a manifest was refused: the key at fault and a short reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackError {
    /// The manifest key (or `line N` for a line that is not `key=value`).
    pub key: String,
    /// What is wrong with it.
    pub reason: &'static str,
}

impl PackError {
    fn new(key: impl Into<String>, reason: &'static str) -> Self {
        Self {
            key: key.into(),
            reason,
        }
    }
}

impl fmt::Display for PackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "`{}`: {}", self.key, self.reason)
    }
}

impl std::error::Error for PackError {}

/// The six ink roles of one face.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InkSet {
    /// Primary text and glyphs.
    pub ink: Rgb,
    /// Secondary text.
    pub muted: Rgb,
    /// Tertiary text.
    pub faint: Rgb,
    /// Slider, meter and route tracks.
    pub track: Rgb,
    /// The quiet track under traces and the tab line.
    pub faint_track: Rgb,
    /// Hairline borders.
    pub border: Rgb,
}

/// What a face is drawn on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Floor {
    /// A two stop colour gradient.
    Colour {
        /// The top stop.
        top: Rgb,
        /// The bottom stop.
        high: Rgb,
    },
    /// A raster material, known by the luminance range of its text zone.
    Material {
        /// The darkest luminance in the text zone.
        lum_min: f64,
        /// The lightest luminance in the text zone.
        lum_max: f64,
    },
}

/// How the flow colour is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowStyle {
    /// Bars and dots filled with the accent.
    Fill,
    /// Levels drawn as holes in the ink.
    Punch,
}

/// The motion a pack opens with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Signature {
    /// Rings (`rings`).
    Rings,
    /// Layers (`layers`).
    Layers,
    /// Tension (`tension`).
    Tension,
    /// Feed (`feed`).
    Feed,
    /// No signature, today's motion (`none`).
    #[default]
    Still,
}

/// One face of the island: its floor and ink.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Face {
    /// What the face is drawn on.
    pub floor: Floor,
    /// The ink drawn on it.
    pub ink: InkSet,
}

/// A parsed, validated theme pack.
#[derive(Debug, Clone, PartialEq)]
pub struct Pack {
    /// Lower case id, the `theme=` value.
    pub id: String,
    /// The product name.
    pub name: String,
    /// The card line per language code.
    pub blurbs: Vec<(String, String)>,
    /// The open island.
    pub open: Face,
    /// The collapsed pill.
    pub collapsed: Face,
    /// The default accent.
    pub flow: Rgb,
    /// How the accent is drawn.
    pub flow_style: FlowStyle,
    /// Whether the user may pick the accent.
    pub flow_adjustable: bool,
    /// Whether the user may pick the floor.
    pub floor_adjustable: bool,
    /// Suggested accents, each passing on the open face.
    pub swatches: Vec<Rgb>,
    /// Where text may sit, as `x, y, w, h` in DIP.
    pub text_zone: Option<[f32; 4]>,
    /// The open motion.
    pub signature: Signature,
    /// Overlay raster names the signature uses.
    pub overlays: Vec<String>,
    /// Whether the floor is dark.
    pub dark: bool,
}

impl Pack {
    /// What a face is drawn against. Never an empty `Flat`.
    #[must_use]
    pub fn backdrop(&self, face: &Face) -> Backdrop {
        match face.floor {
            Floor::Colour { top, high } => Backdrop::Flat {
                lums: vec![top.luminance(), high.luminance()],
            },
            Floor::Material { lum_min, lum_max } => Backdrop::Range {
                min: lum_min,
                max: lum_max,
            },
        }
    }

    /// The card line for a language code, falling back to `en`.
    #[must_use]
    pub fn blurb(&self, lang_code: &str) -> &str {
        let find = |code: &str| {
            self.blurbs
                .iter()
                .find(|(c, _)| c == code)
                .map(|(_, text)| text.as_str())
        };
        find(lang_code).or_else(|| find("en")).unwrap_or("")
    }
}

/// Parses and validates a manifest.
///
/// # Errors
/// Returns the first key at fault: an unknown format, key or duplicate, a bad value, a missing
/// required key, a reserved or contradictory setting, or a failed contrast bar.
pub fn parse_pack(text: &str) -> Result<Pack, PackError> {
    let fields = parse::read(text)?;
    if fields.required("format")? != "1" {
        return Err(PackError::new("format", "unknown format"));
    }
    let id = fields.required("id")?;
    let id_ok = (1..=32).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    if !id_ok {
        return Err(PackError::new("id", "bad id"));
    }
    let name = fields.required("name")?;
    let blurbs: Vec<(String, String)> = fields
        .with_prefix("blurb.")
        .filter(|(code, _)| LANGS.contains(code))
        .map(|(code, line)| (code.to_owned(), line.to_owned()))
        .collect();
    fields.required("blurb.en")?;
    if fields.boolean("material.tintable")? == Some(true) {
        return Err(PackError::new("material.tintable", "tinting is not built"));
    }
    let open = face(&fields, "", None)?;
    let collapsed = face(&fields, "collapsed.", Some(&open))?;
    let floor_adjustable = fields.boolean("floor.adjustable")?.unwrap_or(false);
    if floor_adjustable && matches!(open.floor, Floor::Material { .. }) {
        return Err(PackError::new(
            "floor.adjustable",
            "a material floor is not adjustable",
        ));
    }
    let dark = match fields.boolean("dark")? {
        Some(dark) => dark,
        None => floor_luminance(&open.floor) < DARK_LUMINANCE,
    };
    let pack = Pack {
        id: id.to_owned(),
        name: name.to_owned(),
        blurbs,
        open,
        collapsed,
        flow: fields.colour_required("flow.color")?,
        flow_style: flow_style(&fields)?,
        flow_adjustable: fields.boolean("flow.adjustable")?.unwrap_or(false),
        floor_adjustable,
        swatches: swatches(&fields)?,
        text_zone: text_zone(&fields)?,
        signature: signature(&fields)?,
        overlays: fields
            .list("motion.overlays")?
            .map(|items| items.into_iter().map(str::to_owned).collect())
            .unwrap_or_default(),
        dark,
    };
    check_contrast(&pack)?;
    Ok(pack)
}

fn floor_luminance(floor: &Floor) -> f64 {
    match *floor {
        Floor::Colour { top, .. } => top.luminance(),
        Floor::Material { lum_max, .. } => lum_max,
    }
}

fn flow_style(fields: &Fields) -> Result<FlowStyle, PackError> {
    match fields.get("flow.style") {
        None | Some("fill") => Ok(FlowStyle::Fill),
        Some("punch") => Ok(FlowStyle::Punch),
        Some(_) => Err(PackError::new("flow.style", "bad value")),
    }
}

fn signature(fields: &Fields) -> Result<Signature, PackError> {
    match fields.get("motion.open") {
        None | Some("none") => Ok(Signature::Still),
        Some("rings") => Ok(Signature::Rings),
        Some("layers") => Ok(Signature::Layers),
        Some("tension") => Ok(Signature::Tension),
        Some("feed") => Ok(Signature::Feed),
        Some(_) => Err(PackError::new("motion.open", "bad value")),
    }
}

fn text_zone(fields: &Fields) -> Result<Option<[f32; 4]>, PackError> {
    let Some(items) = fields.list("text_zone")? else {
        return Ok(None);
    };
    parse::rect(&items)
        .map(Some)
        .ok_or_else(|| PackError::new("text_zone", "bad list"))
}

fn swatches(fields: &Fields) -> Result<Vec<Rgb>, PackError> {
    let Some(items) = fields.list("flow.swatches")? else {
        return Ok(Vec::new());
    };
    if items.len() > MAX_SWATCHES {
        return Err(PackError::new("flow.swatches", "more than 6 swatches"));
    }
    items
        .into_iter()
        .map(|item| Rgb::parse(item).ok_or_else(|| PackError::new("flow.swatches", "bad colour")))
        .collect()
}

/// Reads one face. `prefix` is `""` for the open face or `collapsed.`; the collapsed face takes
/// every absent key from `fallback`, the floor as a whole.
fn face(fields: &Fields, prefix: &str, fallback: Option<&Face>) -> Result<Face, PackError> {
    let floor_prefix = format!("{prefix}floor.");
    let floor = match fallback {
        Some(open) if !fields.has_prefix(&floor_prefix) => open.floor,
        _ => floor(fields, &floor_prefix)?,
    };
    let role = |name: &str, from: fn(&InkSet) -> Rgb| {
        let key = format!("{prefix}ink.{name}");
        match (fields.colour(&key)?, fallback) {
            (Some(colour), _) => Ok(colour),
            (None, Some(open)) => Ok(from(&open.ink)),
            (None, None) => Err(PackError::new(key, "missing required key")),
        }
    };
    let ink = InkSet {
        ink: role("primary", |s| s.ink)?,
        muted: role("muted", |s| s.muted)?,
        faint: role("faint", |s| s.faint)?,
        track: role("track", |s| s.track)?,
        faint_track: role("faint_track", |s| s.faint_track)?,
        border: role("border", |s| s.border)?,
    };
    Ok(Face { floor, ink })
}

fn floor(fields: &Fields, p: &str) -> Result<Floor, PackError> {
    let colour = |name: &str| fields.colour_required(&format!("{p}{name}"));
    match fields.required(&format!("{p}kind"))? {
        "color" => Ok(Floor::Colour {
            top: colour("color")?,
            high: colour("color_high")?,
        }),
        "material" => {
            let lum = |name: &str| {
                let key = format!("{p}{name}");
                match fields.number(&key)? {
                    Some(n) if (0.0..=1.0).contains(&n) => Ok(n),
                    Some(_) => Err(PackError::new(key, "bad number")),
                    None => Err(PackError::new(key, "missing required key")),
                }
            };
            let (lum_min, lum_max) = (lum("lum_min")?, lum("lum_max")?);
            if lum_min > lum_max {
                return Err(PackError::new(
                    format!("{p}lum_min"),
                    "lum_min above lum_max",
                ));
            }
            Ok(Floor::Material { lum_min, lum_max })
        }
        _ => Err(PackError::new(format!("{p}kind"), "bad value")),
    }
}

/// Checks every contrast bar: each face's ink roles on its own floor, and the flow accent and
/// the swatches on the open face (the collapsed pill draws only ink).
fn check_contrast(pack: &Pack) -> Result<(), PackError> {
    for (prefix, face) in [("", &pack.open), ("collapsed.", &pack.collapsed)] {
        let backdrop = pack.backdrop(face);
        let roles = [
            ("primary", face.ink.ink, INK_BAR),
            ("muted", face.ink.muted, MUTED_BAR),
            ("faint", face.ink.faint, FAINT_BAR),
        ];
        for (name, colour, bar) in roles {
            if worst_contrast(colour, &backdrop) < bar {
                return Err(PackError::new(
                    format!("{prefix}ink.{name}"),
                    "contrast too low",
                ));
            }
        }
    }
    let backdrop = pack.backdrop(&pack.open);
    if pack.flow_style == FlowStyle::Fill && worst_contrast(pack.flow, &backdrop) < FLOW_BAR {
        return Err(PackError::new("flow.color", "contrast too low"));
    }
    if pack
        .swatches
        .iter()
        .any(|&s| worst_contrast(s, &backdrop) < FLOW_BAR)
    {
        return Err(PackError::new("flow.swatches", "contrast too low"));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
