//! Themes (ADR 0007): fixed palettes over one visual language. A theme sets colours only; shape,
//! type and spacing are the same in every theme.

use crate::draw::Ink;

/// The colours of one theme as straight sRGB RGBA in 0..=1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    /// The floor gradient's top stop.
    pub floor: [f32; 4],
    /// The floor gradient's bottom stop.
    pub floor_high: [f32; 4],
    /// Hairline borders.
    pub border: [f32; 4],
    /// Primary text and glyphs.
    pub ink: [f32; 4],
    /// Secondary text.
    pub muted: [f32; 4],
    /// Tertiary text.
    pub faint: [f32; 4],
    /// Slider, meter and route tracks.
    pub track: [f32; 4],
    /// The quiet track under traces and the tab line.
    pub faint_track: [f32; 4],
    /// The signal-flow colour: sound flows here now, nothing else.
    pub flow: [f32; 4],
}

/// The fixed set of themes.
///
/// The variant names `Gece`, `Kagit`, `York` and `Sodyum` are the product names of the themes
/// as the spec gives them (ADR 0007), kept as proper names on purpose rather than translated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Theme {
    /// Loft dark with a lime flow; the default.
    #[default]
    Gece,
    /// Loft light with a dark green flow.
    Kagit,
    /// A 1951 NX panel green-grey with a lamp white flow.
    York,
    /// Black with an amber flow.
    Sodyum,
}

/// An opaque colour from 8-bit sRGB channels.
#[must_use]
pub fn hex(r: u8, g: u8, b: u8) -> [f32; 4] {
    [
        f32::from(r) / 255.0,
        f32::from(g) / 255.0,
        f32::from(b) / 255.0,
        1.0,
    ]
}

impl Theme {
    /// Every theme, in the order the settings window lists them.
    pub const ALL: [Theme; 4] = [Self::Gece, Self::Kagit, Self::York, Self::Sodyum];

    /// The key shared by the settings file and `--theme`.
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Self::Gece => "gece",
            Self::Kagit => "kagit",
            Self::York => "york",
            Self::Sodyum => "sodyum",
        }
    }

    /// The theme for a key, or `None` when the key is unknown.
    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|theme| theme.key() == key)
    }

    /// Whether the theme has a dark floor (window chrome such as the caption follows it).
    #[must_use]
    pub fn is_dark(self) -> bool {
        !matches!(self, Self::Kagit)
    }

    /// The palette of the theme. Gece is the spec's Loft palette; Kağıt, York and Sodyum come
    /// from the R3 mock, with the raised `faint` values of the spec.
    #[must_use]
    pub fn palette(self) -> Palette {
        match self {
            Self::Gece => Palette {
                floor: hex(0x0B, 0x0B, 0x0A),
                floor_high: hex(0x15, 0x14, 0x12),
                border: hex(0x2B, 0x28, 0x24),
                ink: hex(0xEC, 0xE9, 0xE2),
                muted: hex(0xA3, 0x9E, 0x94),
                faint: hex(0x8A, 0x84, 0x7A),
                track: hex(0x4A, 0x46, 0x3F),
                faint_track: hex(0x2E, 0x2B, 0x27),
                flow: hex(0xCA, 0xFF, 0x33),
            },
            Self::Kagit => Palette {
                floor: hex(0xF3, 0xF2, 0xED),
                floor_high: hex(0xE7, 0xE6, 0xE0),
                border: hex(0xCF, 0xCB, 0xC2),
                ink: hex(0x1A, 0x19, 0x16),
                muted: hex(0x55, 0x51, 0x4A),
                faint: hex(0x6B, 0x66, 0x5E),
                track: hex(0xB3, 0xAE, 0xA4),
                faint_track: hex(0xD6, 0xD2, 0xC9),
                flow: hex(0x3F, 0x6E, 0x00),
            },
            Self::York => Palette {
                floor: hex(0x1C, 0x24, 0x20),
                floor_high: hex(0x22, 0x2B, 0x26),
                border: hex(0x34, 0x40, 0x3A),
                ink: hex(0xE4, 0xEA, 0xE4),
                muted: hex(0x9E, 0xAA, 0xA2),
                faint: hex(0x86, 0x95, 0x8E),
                track: hex(0x46, 0x53, 0x4D),
                faint_track: hex(0x2C, 0x36, 0x31),
                flow: hex(0xFF, 0xE9, 0xA8),
            },
            Self::Sodyum => Palette {
                floor: hex(0x09, 0x09, 0x0A),
                floor_high: hex(0x11, 0x11, 0x13),
                border: hex(0x26, 0x26, 0x2A),
                ink: hex(0xED, 0xED, 0xEF),
                muted: hex(0xA2, 0xA2, 0xA8),
                faint: hex(0x86, 0x86, 0x8D),
                track: hex(0x45, 0x45, 0x4B),
                faint_track: hex(0x2A, 0x2A, 0x2E),
                flow: hex(0xFF, 0xB2, 0x24),
            },
        }
    }
}

impl Default for Palette {
    fn default() -> Self {
        Theme::default().palette()
    }
}

impl Palette {
    /// The installer palette: the icon's lime speaker fabric, the same in every theme. It is a
    /// standalone constructor, not a `Theme`, so the settings theme row never offers it.
    ///
    /// `floor` is the average of an 80 x 80 patch of plain fabric (x 40 to 120, y 300 to 380,
    /// clear of the ripples and the text) in `docs/design/identity/r3/installer/bg-0-1x.png`:
    /// (204, 251, 12). The fabric is flat in colour, so `floor_high` is the same value.
    #[must_use]
    pub fn setup() -> Self {
        Self {
            floor: hex(204, 251, 12),
            floor_high: hex(204, 251, 12),
            border: hex(0x8F, 0xB8, 0x0A),
            ink: hex(0x0A, 0x0D, 0x02),
            muted: hex(0x2E, 0x3A, 0x04),
            faint: hex(0x3B, 0x48, 0x06),
            track: hex(0x8F, 0xB8, 0x0A),
            faint_track: hex(0xB4, 0xE0, 0x0C),
            flow: hex(0xCA, 0xFF, 0x33),
        }
    }

    /// The colour of a role. `Floor` is the floor's top stop and `FloorHigh` its bottom stop.
    #[must_use]
    pub fn resolve(&self, ink: Ink) -> [f32; 4] {
        match ink {
            Ink::Floor => self.floor,
            Ink::FloorHigh => self.floor_high,
            Ink::Border => self.border,
            Ink::Ink => self.ink,
            Ink::Muted => self.muted,
            Ink::Faint => self.faint,
            Ink::Track => self.track,
            Ink::FaintTrack => self.faint_track,
            Ink::Flow => self.flow,
        }
    }
}

/// WCAG relative luminance of an sRGB colour (alpha ignored).
fn luminance(rgba: [f32; 4]) -> f64 {
    let lin = |c: f32| {
        let c = f64::from(c);
        if c <= 0.040_45 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(rgba[0]) + 0.7152 * lin(rgba[1]) + 0.0722 * lin(rgba[2])
}

/// The WCAG contrast ratio of two opaque colours, from 1 to 21, in either order.
#[must_use]
pub fn contrast(a: [f32; 4], b: [f32; 4]) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

#[cfg(test)]
mod tests;
