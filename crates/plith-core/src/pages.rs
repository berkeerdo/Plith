//! Page layouts: pure functions that return display list nodes and hit targets for the render
//! thread and the pointer router.

use crate::draw::{DisplayList, Rect};
use crate::geometry::IslandShape;
use crate::lang::{Lang, Text, text};
use crate::settings::IdleLook;

pub mod audio;
pub mod copy;
pub mod derive;
pub mod fixtures;
mod glyphs;
pub mod idle_pill;
mod kit;
pub mod media;
pub mod pill;
pub mod quick;
pub mod shelf;
pub mod system;
pub mod tabbar;
pub mod welcome;

/// Page width in DIP.
pub const PAGE_W: f64 = 620.0;
/// Left margin of page content in DIP.
pub const MARGIN_LEFT: f64 = 30.0;
/// Right margin of page content in DIP.
pub const MARGIN_RIGHT: f64 = 24.0;

/// Number of pages: the four tab pages, the welcome view and the quick page.
pub const PAGE_COUNT: usize = 6;

/// One of the pages: four with a tab, and the welcome view with none.
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
    /// The welcome view: the island's opening page, with no tab.
    Welcome,
    /// The quick settings page behind the gear mark, with no tab.
    Quick,
}

impl PageId {
    /// Every page: the tab pages in tab order, then the welcome view and the quick page.
    pub const ALL: [PageId; PAGE_COUNT] = [
        PageId::Audio,
        PageId::Media,
        PageId::Shelf,
        PageId::System,
        PageId::Welcome,
        PageId::Quick,
    ];

    /// The pages that have a tab, in tab order.
    pub const TABS: [PageId; 4] = [PageId::Audio, PageId::Media, PageId::Shelf, PageId::System];

    /// The page position, 0 to 5; the tab pages come first, so a tab position is its page position.
    #[must_use]
    pub fn index(self) -> usize {
        match self {
            PageId::Audio => 0,
            PageId::Media => 1,
            PageId::Shelf => 2,
            PageId::System => 3,
            PageId::Welcome => 4,
            PageId::Quick => 5,
        }
    }

    /// The page at a position, or `None` past the last page.
    #[must_use]
    pub fn from_index(index: usize) -> Option<PageId> {
        Self::ALL.get(index).copied()
    }

    /// The tab label shown to the user in `lang`; empty for the pages with no tab.
    #[must_use]
    pub fn label(self, lang: Lang) -> &'static str {
        match self {
            PageId::Audio => text(Text::TabAudio, lang),
            PageId::Media => text(Text::TabMedia, lang),
            PageId::Shelf => text(Text::TabShelf, lang),
            PageId::System => text(Text::TabSystem, lang),
            PageId::Welcome | PageId::Quick => "",
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
    /// Set the volume of the default output to this percentage (0 to 100).
    SetVolume(u8),
    /// A press on the volume ruler: `hit` turns it into `SetVolume` at the click.
    VolumeRuler(Rect),
    /// Set the brightness of the system monitor to this percentage (0 to 100).
    SetBrightness(u8),
    /// A press on the brightness ruler: `hit` turns it into `SetBrightness` at the click.
    BrightnessRuler(Rect),
    /// Mute or unmute the capture device.
    ToggleMic,
    /// Pick this resting look (on the quick page); applies at once.
    Look(IdleLook),
    /// Open the settings window (on the quick page).
    OpenSettings,
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
    target_at(targets, x, y).map(|t| match &t.action {
        Action::SeekRuler(ruler) => Action::Seek(seek_fraction(*ruler, x)),
        Action::VolumeRuler(ruler) => Action::SetVolume(system::brightness_at(*ruler, x)),
        Action::BrightnessRuler(ruler) => Action::SetBrightness(system::brightness_at(*ruler, x)),
        other => other.clone(),
    })
}

/// The first target whose rect, grown by 4 DIP on each side, contains the point.
pub(crate) fn target_at(targets: &[HitTarget], x: f64, y: f64) -> Option<&HitTarget> {
    targets.iter().find(|t| {
        x >= t.rect.x - HIT_SLACK
            && x <= t.rect.x + t.rect.w + HIT_SLACK
            && y >= t.rect.y - HIT_SLACK
            && y <= t.rect.y + t.rect.h + HIT_SLACK
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
