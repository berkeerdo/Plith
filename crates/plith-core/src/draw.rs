//! Display lists: what the UI thread builds for a page and the render thread paints. Types only;
//! painting lives in the render crate.

use std::sync::Arc;
use std::time::SystemTime;

/// A named colour role; the painter maps it to a colour (and the content opacity).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Ink {
    /// The darkest panel fill.
    Floor,
    /// A panel fill one step above the floor.
    FloorHigh,
    /// Hairline borders.
    Border,
    /// Primary text and glyphs.
    Ink,
    /// Secondary text.
    Muted,
    /// Tertiary text.
    Faint,
    /// Slider and meter tracks.
    Track,
    /// The signal-flow accent.
    Flow,
}

/// A rectangle in page DIP, relative to the page origin.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    /// Left edge.
    pub x: f64,
    /// Top edge.
    pub y: f64,
    /// Width.
    pub w: f64,
    /// Height.
    pub h: f64,
}

/// A text role: size, weight and font are the painter's business.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextStyle {
    /// A page title.
    Title,
    /// An item name.
    Name,
    /// A secondary line.
    Sub,
    /// A small caps label.
    Label,
    /// A numeric value.
    Value,
    /// A clock readout.
    Clock,
}

/// Where an overlong line loses its characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Trim {
    /// Cut the end.
    End,
    /// Cut the middle.
    Middle,
}

/// Horizontal text alignment inside its rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Align {
    /// Flush with the leading edge.
    Start,
    /// Flush with the trailing edge.
    End,
    /// Centred.
    Center,
}

/// A station marker on a signal-flow line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StationMark {
    /// A filled dot.
    Filled,
    /// A hollow ring.
    Hollow,
}

/// Which family of live meters a live node reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LiveRail {
    /// The hardware input strips.
    Strip,
    /// The output buses.
    Bus,
}

/// Identity of a piece of artwork.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ArtId(pub u64);

/// Decoded artwork: premultiplied BGRA rows of `width * 4` bytes each.
#[derive(Debug, Clone, PartialEq)]
pub struct ArtPixels {
    /// The artwork identity.
    pub id: ArtId,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// The pixel rows.
    pub bgra: Arc<[u8]>,
}

/// A playback position the painter extrapolates between media updates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SeekTimeline {
    /// The position in seconds at `stamp`.
    pub position_s: f64,
    /// The track length in seconds.
    pub duration_s: f64,
    /// When `position_s` was read.
    pub stamp: SystemTime,
    /// Whether the position advances.
    pub playing: bool,
}

/// A node whose look changes between display lists without the list being rebuilt.
#[derive(Debug, Clone, PartialEq)]
pub enum LiveNode {
    /// A level meter.
    Meter {
        /// Where it is drawn.
        rect: Rect,
        /// The meter family.
        rail: LiveRail,
        /// Which meter of the family.
        index: usize,
    },
    /// A level history trace.
    Trace {
        /// Where it is drawn.
        rect: Rect,
        /// The meter family.
        rail: LiveRail,
        /// Which meter of the family.
        index: usize,
    },
    /// The seek bar.
    Seek {
        /// Where it is drawn.
        rect: Rect,
        /// The playback position to extrapolate.
        timeline: SeekTimeline,
    },
}

/// One drawing command.
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    /// A filled rounded rectangle.
    Fill {
        /// Where it is drawn.
        rect: Rect,
        /// Corner radius.
        radius: f64,
        /// The colour role.
        ink: Ink,
    },
    /// A polyline.
    Stroke {
        /// The vertices.
        points: Vec<(f64, f64)>,
        /// Line width.
        width: f64,
        /// The colour role.
        ink: Ink,
        /// Whether the line is dashed.
        dashed: bool,
    },
    /// A station marker.
    Station {
        /// Centre x.
        x: f64,
        /// Centre y.
        y: f64,
        /// Filled or hollow.
        mark: StationMark,
        /// The colour role.
        ink: Ink,
    },
    /// A line of text.
    Text {
        /// Where it is drawn.
        rect: Rect,
        /// The text.
        text: Arc<str>,
        /// The text role.
        style: TextStyle,
        /// The colour role.
        ink: Ink,
        /// Alignment inside `rect`.
        align: Align,
        /// How an overlong line is cut.
        trim: Trim,
    },
    /// Artwork scaled into a rectangle.
    Image {
        /// Where it is drawn.
        rect: Rect,
        /// Which artwork.
        art: ArtId,
    },
    /// A live node.
    Live(LiveNode),
}

/// The commands of one page or pill and the size they are laid out for.
#[derive(Debug, Clone, PartialEq)]
pub struct DisplayList {
    /// The commands, painted in order.
    pub nodes: Vec<Node>,
    /// Width and height in DIP.
    pub size: (f64, f64),
}

impl DisplayList {
    /// A list with no commands and size (0, 0).
    #[must_use]
    pub fn empty() -> Self {
        Self {
            nodes: Vec::new(),
            size: (0.0, 0.0),
        }
    }
}

/// Everything the painter needs besides the scene.
#[derive(Debug, Clone, PartialEq)]
pub struct PageContent {
    /// One list per page.
    pub pages: [Arc<DisplayList>; 4],
    /// The collapsed pill.
    pub pill: Arc<DisplayList>,
    /// Artwork referenced by `Node::Image`.
    pub art: Vec<Arc<ArtPixels>>,
}

impl PageContent {
    /// Four empty pages, an empty pill and no artwork.
    #[must_use]
    pub fn empty() -> Self {
        let empty = || Arc::new(DisplayList::empty());
        Self {
            pages: [empty(), empty(), empty(), empty()],
            pill: empty(),
            art: Vec::new(),
        }
    }
}

impl Default for PageContent {
    fn default() -> Self {
        Self::empty()
    }
}
