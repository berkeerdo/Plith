//! Display lists: what the UI thread builds for a page and the render thread paints. Types only;
//! painting lives in the render crate.

use crate::pages::PAGE_COUNT;
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
    /// The fainter track of a tab bar hairline.
    FaintTrack,
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
    /// Running text: regular sans 12.
    Body,
    /// A strong line: semibold sans 14.
    Strong,
    /// The welcome view clock: mono 30.
    Hero,
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

impl SeekTimeline {
    /// The played fraction (0 to 1) at wall time `now`: the position plus the time since the
    /// stamp while playing (none when `now` is before it), over the duration. 0 for a track
    /// with no length.
    #[must_use]
    pub fn fraction_at(&self, now: SystemTime) -> f64 {
        if self.duration_s.is_nan() || self.duration_s <= 0.0 {
            return 0.0;
        }
        (self.position_at(now) / self.duration_s).clamp(0.0, 1.0)
    }

    /// The position in seconds at wall time `now`: the position plus the time since the stamp
    /// while playing (none when `now` is before it), not past a positive duration.
    #[must_use]
    pub fn position_at(&self, now: SystemTime) -> f64 {
        let elapsed = if self.playing {
            now.duration_since(self.stamp)
                .map_or(0.0, |gap| gap.as_secs_f64())
        } else {
            0.0
        };
        let position = self.position_s + elapsed;
        if self.duration_s > 0.0 {
            position.min(self.duration_s)
        } else {
            position
        }
    }
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
        /// The colour role of the played part and its head (core decides it, the painter maps
        /// it).
        ink: Ink,
    },
    /// The elapsed time of a playing track, as text that advances with the clock. The painter
    /// lays out the current string every frame (its layout cache keeps one per string), so the
    /// page bitmap does not change while the track plays.
    Elapsed {
        /// Where it is drawn.
        rect: Rect,
        /// The playback position to extrapolate.
        timeline: SeekTimeline,
        /// The text style.
        style: TextStyle,
        /// The colour role.
        ink: Ink,
        /// The alignment inside `rect`.
        align: Align,
    },
    /// A title that scrolls left once from `start`, then rests at the start. Drawn left-aligned
    /// from `rect.x`; `run_s` is core's estimate of the run, the painter clamps to the measured
    /// overflow.
    Scroll {
        /// The visible box.
        rect: Rect,
        /// The whole title.
        text: Arc<str>,
        /// The text style.
        style: TextStyle,
        /// The colour role.
        ink: Ink,
        /// When the run began.
        start: SystemTime,
        /// The estimated run length in seconds.
        run_s: f64,
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
    /// A filled polygon (closed implicitly), such as a transport triangle.
    Polygon {
        /// The vertices.
        points: Vec<(f64, f64)>,
        /// The colour role.
        ink: Ink,
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
    /// A list drawn at `(x, y)` scaled by `scale`, for true-size previews. Its live nodes are
    /// skipped.
    Inset {
        /// The list drawn inside.
        list: Arc<DisplayList>,
        /// Where its origin lands.
        x: f64,
        /// Where its origin lands.
        y: f64,
        /// The uniform scale.
        scale: f64,
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
    pub pages: [Arc<DisplayList>; PAGE_COUNT],
    /// The collapsed pill.
    pub pill: Arc<DisplayList>,
    /// Artwork referenced by `Node::Image`.
    pub art: Vec<Arc<ArtPixels>>,
}

impl PageContent {
    /// Five empty pages, an empty pill and no artwork.
    #[must_use]
    pub fn empty() -> Self {
        let empty = || Arc::new(DisplayList::empty());
        Self {
            pages: std::array::from_fn(|_| empty()),
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

/// Which bundled font family a text role uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Family {
    /// The proportional face (Public Sans).
    Sans,
    /// The monospaced face (Martian Mono).
    Mono,
}

/// The typographic recipe of one text role.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StyleSpec {
    /// The font family.
    pub family: Family,
    /// The font size in DIP.
    pub size_dip: f32,
    /// The font weight (400 regular, 500 medium, 600 semibold).
    pub weight: u16,
    /// Whether the renderer upper-cases the text before layout.
    pub uppercase: bool,
    /// Extra spacing after every character, in em.
    pub tracking_em: f32,
}

/// Letter spacing of every mono role, in em (the mock's `letter-spacing: 0.06em`).
const MONO_TRACKING: f32 = 0.06;

/// The font, size, weight and tracking of a text role.
#[must_use]
pub fn style_spec(style: TextStyle) -> StyleSpec {
    let (family, size_dip, weight, uppercase, tracking_em) = match style {
        TextStyle::Title => (Family::Sans, 16.0, 600, false, 0.0),
        TextStyle::Name => (Family::Sans, 13.0, 400, false, 0.0),
        // Sub and Label share the mock's look; the roles stay apart for their callers.
        TextStyle::Sub | TextStyle::Label => (Family::Mono, 10.0, 400, true, MONO_TRACKING),
        TextStyle::Value => (Family::Mono, 10.0, 400, false, MONO_TRACKING),
        TextStyle::Clock => (Family::Mono, 12.0, 400, false, MONO_TRACKING),
        TextStyle::Body => (Family::Sans, 12.0, 400, false, 0.0),
        TextStyle::Strong => (Family::Sans, 14.0, 600, false, 0.0),
        TextStyle::Hero => (Family::Mono, 30.0, 400, false, MONO_TRACKING),
    };
    StyleSpec {
        family,
        size_dip,
        weight,
        uppercase,
        tracking_em,
    }
}

#[cfg(test)]
mod tests;
