//! Page layouts: pure functions that return display list nodes and hit targets for the render
//! thread and the pointer router.

use crate::draw::Rect;

pub mod audio;
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
            PageId::Audio => "SES",
            PageId::Media => "MEDYA",
            PageId::Shelf => "RAF",
            PageId::System => "S\u{130}STEM",
        }
    }
}

/// What a pointer press on a hit target does.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// Switch to a page.
    Tab(PageId),
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
        .map(|t| t.action.clone())
}
