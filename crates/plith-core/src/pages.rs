//! Page layouts: pure functions that return display list nodes and hit targets for the render
//! thread and the pointer router.

use crate::draw::{DisplayList, Rect};
use crate::geometry::IslandShape;

pub mod audio;
pub mod copy;
pub mod derive;
pub mod fixtures;
mod kit;
pub mod media;
pub mod pill;
pub mod shelf;
pub mod system;
pub mod tabbar;

/// Page width in DIP.
pub const PAGE_W: f64 = 620.0;
/// Left margin of page content in DIP.
pub const MARGIN_LEFT: f64 = 30.0;
/// Right margin of page content in DIP.
pub const MARGIN_RIGHT: f64 = 24.0;

/// One of the four pages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PageId {
    /// The audio routing page.
    Audio,
    /// The media page.
    Media,
    /// The shelf page.
    Shelf,
    /// The system controls page.
    System,
}

impl PageId {
    /// Every page in tab order.
    pub const ALL: [PageId; 4] = [PageId::Audio, PageId::Media, PageId::Shelf, PageId::System];

    /// The tab position, 0 to 3.
    #[must_use]
    pub fn index(self) -> usize {
        match self {
            PageId::Audio => 0,
            PageId::Media => 1,
            PageId::Shelf => 2,
            PageId::System => 3,
        }
    }

    /// The page at a tab position, or `None` past the last tab.
    #[must_use]
    pub fn from_index(index: usize) -> Option<PageId> {
        Self::ALL.get(index).copied()
    }

    /// The tab label shown to the user.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            PageId::Audio => copy::TAB_AUDIO,
            PageId::Media => copy::TAB_MEDIA,
            PageId::Shelf => copy::TAB_SHELF,
            PageId::System => copy::TAB_SYSTEM,
        }
    }
}

/// What a pointer press on a hit target does.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// Switch to a page.
    Tab(PageId),
    /// Toggle play and pause.
    PlayPause,
    /// Skip to the next track.
    Next,
    /// Skip to the previous track.
    Previous,
    /// Seek to a fraction (0 to 1) of the track.
    Seek(f64),
    /// A press on a seek ruler: `hit` turns it into `Seek` with the fraction at the click.
    SeekRuler(Rect),
    /// Pick the source at this audio row index as a route entrance.
    PickSource(usize),
    /// Set a route from the picked source to the output at this row index.
    PickOutput(usize),
    /// Leave pick mode without setting a route.
    CancelPick,
    /// Make the Windows endpoint with this id the default output.
    SetDefaultEndpoint(String),
    /// Set the brightness of the system monitor to this percentage (0 to 100).
    SetBrightness(u8),
    /// A press on the brightness ruler: `hit` turns it into `SetBrightness` at the click.
    BrightnessRuler(Rect),
    /// Mute or unmute the capture device.
    ToggleMic,
}

/// A built page: what to draw, where to click and how big the island is.
#[derive(Debug, Clone, PartialEq)]
pub struct PageBuild {
    /// The display list, sized to the page.
    pub list: DisplayList,
    /// The hit targets, tab bar included.
    pub targets: Vec<HitTarget>,
    /// The island shape that holds the page.
    pub shape: IslandShape,
}

/// A clickable area and its action.
#[derive(Debug, Clone, PartialEq)]
pub struct HitTarget {
    /// The area in page DIP.
    pub rect: Rect,
    /// What a press does.
    pub action: Action,
}

/// Slack added around every target when testing a point, in DIP.
const HIT_SLACK: f64 = 4.0;

/// The action of the first target whose rect, grown by 4 DIP on each side, contains the point.
/// A `SeekRuler` target comes back as `Seek` with the fraction at `x`.
#[must_use]
pub fn hit(targets: &[HitTarget], x: f64, y: f64) -> Option<Action> {
    targets
        .iter()
        .find(|t| {
            x >= t.rect.x - HIT_SLACK
                && x <= t.rect.x + t.rect.w + HIT_SLACK
                && y >= t.rect.y - HIT_SLACK
                && y <= t.rect.y + t.rect.h + HIT_SLACK
        })
        .map(|t| match &t.action {
            Action::SeekRuler(ruler) => Action::Seek(seek_fraction(*ruler, x)),
            Action::BrightnessRuler(ruler) => {
                Action::SetBrightness(system::brightness_at(*ruler, x))
            }
            other => other.clone(),
        })
}

/// The played fraction for a click at `x` on `ruler`, clamped to 0..1.
#[must_use]
pub fn seek_fraction(ruler: Rect, x: f64) -> f64 {
    if ruler.w <= 0.0 {
        return 0.0;
    }
    ((x - ruler.x) / ruler.w).clamp(0.0, 1.0)
}
