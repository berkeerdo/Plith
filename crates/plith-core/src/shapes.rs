//! The one constants table: island shapes and surface sizing (all DIP). Colours live in `theme`.

use crate::geometry::IslandShape;

/// The collapsed island.
pub const REST: IslandShape = IslandShape {
    width: 200.0,
    height: 32.0,
    radius: 16.0,
};

/// The four pages: audio, media, shelf and system. These are pre-feed defaults only: the feed
/// publishes the shapes of the built pages at once (`pages::*::*_page().shape`).
pub const PAGES: [IslandShape; 4] = [
    IslandShape {
        width: 620.0,
        height: 300.0,
        radius: 24.0,
    },
    IslandShape {
        width: 620.0,
        height: 232.0,
        radius: 24.0,
    },
    IslandShape {
        width: 620.0,
        height: 170.0,
        radius: 24.0,
    },
    IslandShape {
        width: 620.0,
        height: 178.0,
        radius: 24.0,
    },
];

/// The widest page, in DIP; the surface is sized for it.
pub const PAGE_MAX_WIDTH: f64 = 620.0;
/// The tallest page the surface has room for, in DIP.
pub const PAGE_MAX_HEIGHT: f64 = 420.0;

/// Gap between the work-area top and the island, in DIP: zero, the island is flush with the top.
pub const TOP_GAP: f64 = 0.0;
/// Empty margin around the largest page on the surface, in DIP.
pub const SURFACE_MARGIN: f64 = 20.0;
/// Room reserved for spring overshoot, as a factor of the page size.
pub const OVERSHOOT: f64 = 1.05;
