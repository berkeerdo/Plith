//! The one constants table: island shapes, surface sizing and colours (all DIP).

use crate::geometry::IslandShape;

/// The collapsed island.
pub const REST: IslandShape = IslandShape {
    width: 200.0,
    height: 32.0,
    radius: 16.0,
};

/// The three pages of the slice.
pub const PAGES: [IslandShape; 3] = [
    IslandShape {
        width: 620.0,
        height: 216.0,
        radius: 40.0,
    },
    IslandShape {
        width: 620.0,
        height: 320.0,
        radius: 40.0,
    },
    IslandShape {
        width: 480.0,
        height: 180.0,
        radius: 40.0,
    },
];

/// Gap between the work-area top and the island, in DIP.
pub const TOP_GAP: f64 = 8.0;
/// Empty margin around the largest page on the surface, in DIP.
pub const SURFACE_MARGIN: f64 = 20.0;
/// Room reserved for spring overshoot, as a factor of the page size.
pub const OVERSHOOT: f64 = 1.05;
/// Island fill colour as straight RGBA.
pub const FILL: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

/// A page placeholder: a rounded bar drawn near the island top.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placeholder {
    /// Bar width in DIP.
    pub width: f64,
    /// Bar height in DIP.
    pub height: f64,
    /// Bar corner radius in DIP.
    pub radius: f64,
    /// Distance below the island top in DIP.
    pub top: f64,
    /// Bar colour as straight RGBA.
    pub rgba: [f32; 4],
}

/// The page placeholder bar.
pub const PLACEHOLDER: Placeholder = Placeholder {
    width: 120.0,
    height: 12.0,
    radius: 6.0,
    top: 24.0,
    rgba: [1.0, 1.0, 1.0, 0.16],
};
