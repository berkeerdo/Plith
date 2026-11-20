//! The one constants table: island shapes and surface sizing (all DIP). Colours live in `theme`.

use crate::geometry::IslandShape;
use crate::pages::PAGE_COUNT;

/// The collapsed island.
pub const REST: IslandShape = IslandShape {
    width: 200.0,
    height: 32.0,
    radius: 16.0,
};

/// The collapsed island when the look is `Hidden`: a 3 DIP sliver, no rounding.
pub const REST_HIDDEN: IslandShape = IslandShape {
    width: 200.0,
    height: 3.0,
    radius: 0.0,
};

/// The pages: audio, media, shelf, system, the welcome view and the quick page. These are
/// pre-feed defaults
/// only: the feed publishes the shapes of the built pages at once (`pages::*::*_page().shape`).
pub const PAGES: [IslandShape; PAGE_COUNT] = [
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
    WELCOME,
    QUICK,
];

/// The welcome view, the fifth page: the island's opening view, with no tab.
pub const WELCOME: IslandShape = IslandShape {
    width: 620.0,
    height: 232.0,
    radius: 24.0,
};

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

/// The quick settings page behind the gear mark: the tab row, the closed island caption, six look
/// previews, a rule and one row that opens the settings window.
pub const QUICK: IslandShape = IslandShape {
    width: 620.0,
    height: 208.0,
    radius: 24.0,
};
