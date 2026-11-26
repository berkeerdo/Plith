//! Themes (ADR 0022): a theme is a pack, a manifest plus rasters, resolved through the
//! [`registry`] with the user's accent and floor picks. The solve keeps text and accents readable
//! on the resulting floor. [`Palette`] is the role table the painters read; shape, type and
//! spacing are the same in every theme.

use crate::draw::Ink;

pub mod builtin;
pub mod colour;
#[cfg(test)]
mod fixtures;
pub mod pack;
pub mod registry;
pub mod solve;

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

impl Default for Palette {
    fn default() -> Self {
        use registry::{Picks, Registry, ThemeId};
        let picks = Picks {
            accent: None,
            floor: None,
            windows_accent: None,
        };
        Registry::builtin().resolve(&ThemeId::gece(), picks).open
    }
}

impl Palette {
    /// The installer palette: the icon's lime speaker fabric, the same in every theme. It is a
    /// standalone constructor, not a theme pack, so the settings theme row never offers it.
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
pub(crate) fn luminance(rgba: [f32; 4]) -> f64 {
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
