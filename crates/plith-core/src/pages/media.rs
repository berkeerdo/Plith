//! The media page: art, title, seek ruler, transport and the output route.
//!
//! Positions come from `mediaPage()` and `mediaEmpty()` in `docs/design/r3/ray-paneli.html`.
//! The mock's text baselines become top-aligned rects (baseline minus the cap height).

use super::kit::{line, rect, station, text};
pub use super::seek_fraction;
use super::tabbar::tabbar;
use super::{Action, HitTarget, PAGE_W, PageBuild, PageId};
use crate::draw::{
    Align, ArtId, DisplayList, Ink, LiveNode, Node, Rect, SeekTimeline, StationMark, TextStyle,
};
use crate::shapes;
use std::time::SystemTime;

/// Everything the media page needs to know.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MediaFacts {
    /// Whether a media session exists.
    pub has_session: bool,
    /// The source app name (for example `Spotify`).
    pub source_name: String,
    /// The track title (may be empty).
    pub title: String,
    /// The artist (may be empty).
    pub artist: String,
    /// The decoded cover art, when there is some.
    pub art: Option<ArtId>,
    /// Whether the track is playing.
    pub playing: bool,
    /// Whether the session accepts seeking.
    pub can_seek: bool,
    /// Whether the session can skip forward.
    pub can_next: bool,
    /// Whether the session can skip back.
    pub can_prev: bool,
    /// The playback position, when the session reports one.
    pub timeline: Option<SeekTimeline>,
    /// The output route text after the arrow (for example `A1 · G733`).
    pub output: Option<String>,
}

/// The ruler rectangle, used for the seek hit area and the seek fraction.
pub const RULER: Rect = Rect {
    x: 146.0,
    y: 123.0,
    w: 444.0,
    h: 10.0,
};
/// Page height with a session (mock `mediaPage`, `h = 232`).
const HEIGHT: f64 = 232.0;
/// Page height with no session (mock `mediaEmpty`, `h = 150`).
const HEIGHT_EMPTY: f64 = 150.0;
/// The ruler line y (mock `y = 128`).
const RULER_Y: f64 = 128.0;
/// Ruler divisions (mock: 20, a taller tick every 5).
const DIVISIONS: u8 = 20;
/// Width of the played part of the seek bar (the painter's live seek width).
const SEEK_W: f64 = 2.0;
/// Extra reach of the seek target above and below the ruler.
const SEEK_GROW: f64 = 6.0;
/// Transport centre y (mock `ty = 150`).
const TRANSPORT_Y: f64 = 150.0;
/// Transport bar and outline width (the mock's default 1 DIP stroke).
const GLYPH_W: f64 = 1.0;
/// Route row y with a session (mock `ry = 196`).
const ROUTE_Y: f64 = 196.0;
/// Route row y with no session (mock: 124).
const ROUTE_Y_EMPTY: f64 = 124.0;
/// Route line start x and end x (mock 96 and 380).
const ROUTE_X0: f64 = 96.0;
const ROUTE_X1: f64 = 380.0;

/// `m:ss`, or `h:mm:ss` from one hour.
#[must_use]
pub fn format_time(secs: u64) -> String {
    let (h, m, s) = (secs / 3600, secs / 60 % 60, secs % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// The elapsed time of `timeline` at wall time `now`, as the media page writes it (`m:ss`).
#[must_use]
pub fn elapsed_text(timeline: &SeekTimeline, now: SystemTime) -> String {
    format_time(whole_secs(timeline.position_at(now)))
}

/// Whole seconds of a position; negative and non-finite values become 0.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn whole_secs(s: f64) -> u64 {
    if s.is_finite() && s > 0.0 {
        s as u64
    } else {
        0
    }
}

/// The media page nodes, hit targets and island shape.
#[must_use]
pub fn media_page(facts: &MediaFacts, clock: &str) -> PageBuild {
    let (mut nodes, mut targets) = tabbar(PageId::Media, clock);
    let height = if facts.has_session {
        session_body(facts, &mut nodes, &mut targets);
        HEIGHT
    } else {
        empty_body(facts, &mut nodes);
        HEIGHT_EMPTY
    };
    let mut shape = shapes::PAGES[PageId::Media.index()];
    shape.width = PAGE_W;
    shape.height = height;
    PageBuild {
        list: DisplayList {
            nodes,
            size: (PAGE_W, height),
        },
        targets,
        shape,
    }
}

/// The route row: `SOURCE` label, the line between two stations and the output text. Flow ink
/// while playing, Track and Faint otherwise (and in the empty state, hollow stations).
fn route_row(nodes: &mut Vec<Node>, y: f64, source: Option<&str>, output: &str, lit: bool) {
    if let Some(source) = source {
        nodes.push(text(
            rect(30.0, y - 6.0, 62.0, 12.0),
            source,
            TextStyle::Label,
            Ink::Muted,
            Align::Start,
        ));
    }
    let (line_ink, width) = if lit {
        (Ink::Flow, 2.0)
    } else {
        (Ink::Track, 1.0)
    };
    nodes.push(line(vec![(ROUTE_X0, y), (ROUTE_X1, y)], width, line_ink));
    let (mark, ink) = if lit {
        (StationMark::Filled, Ink::Flow)
    } else {
        (StationMark::Hollow, Ink::Faint)
    };
    nodes.push(station(ROUTE_X0, y, mark, ink));
    nodes.push(station(ROUTE_X1, y, mark, ink));
    let text_ink = if lit { Ink::Ink } else { Ink::Faint };
    nodes.push(text(
        rect(392.0, y - 6.0, PAGE_W - 30.0 - 392.0, 12.0),
        output,
        TextStyle::Label,
        text_ink,
        Align::Start,
    ));
}

fn empty_body(facts: &MediaFacts, nodes: &mut Vec<Node>) {
    nodes.push(text(
        rect(30.0, 73.0, 560.0, 20.0),
        "\u{c7}alan bir \u{15f}ey yok",
        TextStyle::Strong,
        Ink::Ink,
        Align::Start,
    ));
    nodes.push(text(
        rect(30.0, 94.0, 560.0, 12.0),
        "Spotify, taray\u{131}c\u{131} ya da bir oyun ses verdi\u{11f}inde burada g\u{f6}r\u{fc}n\u{fc}r.",
        TextStyle::Body,
        Ink::Muted,
        Align::Start,
    ));
    if let Some(output) = &facts.output {
        route_row(nodes, ROUTE_Y_EMPTY, None, output, false);
    }
}

fn session_body(facts: &MediaFacts, nodes: &mut Vec<Node>, targets: &mut Vec<HitTarget>) {
    let art = rect(30.0, 62.0, 96.0, 96.0);
    nodes.push(match facts.art {
        Some(art_id) => Node::Image {
            rect: art,
            art: art_id,
        },
        None => Node::Fill {
            rect: art,
            radius: 0.0,
            ink: Ink::FaintTrack,
        },
    });
    let title = if facts.title.is_empty() {
        &facts.source_name
    } else {
        &facts.title
    };
    nodes.push(text(
        rect(146.0, 67.5, 444.0, 22.0),
        title,
        TextStyle::Title,
        Ink::Ink,
        Align::Start,
    ));
    if !facts.artist.is_empty() {
        nodes.push(text(
            rect(146.0, 90.5, 444.0, 16.0),
            &facts.artist,
            TextStyle::Body,
            Ink::Muted,
            Align::Start,
        ));
    }
    // The transport targets go first so their top edge wins where the seek target grows into it.
    let seek = facts
        .timeline
        .filter(|t| t.duration_s.is_finite() && t.duration_s > 0.0)
        .and_then(|timeline| ruler(facts, timeline, nodes));
    transport(facts, nodes, targets);
    targets.extend(seek);
    if let Some(output) = &facts.output {
        route_row(
            nodes,
            ROUTE_Y,
            Some(&facts.source_name),
            output,
            facts.playing,
        );
    }
}

fn ruler(facts: &MediaFacts, timeline: SeekTimeline, nodes: &mut Vec<Node>) -> Option<HitTarget> {
    let (x0, w) = (RULER.x, RULER.w);
    nodes.push(line(
        vec![(x0, RULER_Y), (x0 + w, RULER_Y)],
        1.0,
        Ink::Track,
    ));
    for i in 0..=DIVISIONS {
        let x = x0 + w * f64::from(i) / f64::from(DIVISIONS);
        let len = if i % 5 == 0 { 10.0 } else { 7.0 };
        nodes.push(line(
            vec![(x, RULER_Y + 4.0), (x, RULER_Y + len)],
            1.0,
            Ink::Faint,
        ));
    }
    played(timeline, nodes);
    elapsed(timeline, nodes);
    nodes.push(text(
        rect(x0 + w - 80.0, 144.0, 80.0, 12.0),
        &format_time(whole_secs(timeline.duration_s)),
        TextStyle::Sub,
        Ink::Muted,
        Align::End,
    ));
    facts.can_seek.then(|| HitTarget {
        rect: rect(x0, RULER.y - SEEK_GROW, w, RULER.h + 2.0 * SEEK_GROW),
        action: Action::SeekRuler(RULER),
    })
}

/// The played part of the track with a filled station at its head. While playing it moves, so
/// it is a live node in the flow ink (sound flows); paused nothing moves, so it is drawn
/// statically in ink and the page wants no ambient frames.
fn played(timeline: SeekTimeline, nodes: &mut Vec<Node>) {
    let (x0, w) = (RULER.x, RULER.w);
    if timeline.playing {
        nodes.push(Node::Live(LiveNode::Seek {
            rect: rect(x0, RULER_Y - 5.0, w, 10.0),
            timeline,
            ink: Ink::Flow,
        }));
        return;
    }
    // Paused, the fraction does not depend on the clock.
    let head = x0 + w * timeline.fraction_at(timeline.stamp);
    if head > x0 {
        nodes.push(line(vec![(x0, RULER_Y), (head, RULER_Y)], SEEK_W, Ink::Ink));
    }
    nodes.push(station(head, RULER_Y, StationMark::Filled, Ink::Ink));
}

/// The elapsed time under the ruler's start. While playing it advances, so it is a live node
/// and the page list stays equal for the whole track; paused it is static text.
fn elapsed(timeline: SeekTimeline, nodes: &mut Vec<Node>) {
    let place = rect(RULER.x, 144.0, 80.0, 12.0);
    if timeline.playing {
        nodes.push(Node::Live(LiveNode::Elapsed {
            rect: place,
            timeline,
            style: TextStyle::Sub,
            ink: Ink::Muted,
            align: Align::Start,
        }));
        return;
    }
    nodes.push(text(
        place,
        &elapsed_text(&timeline, timeline.stamp),
        TextStyle::Sub,
        Ink::Muted,
        Align::Start,
    ));
}

/// A filled triangle with a 1 DIP outline of the same ink, like the mock's filled and stroked
/// paths.
fn glyph(nodes: &mut Vec<Node>, points: Vec<(f64, f64)>, ink: Ink) {
    let mut outline = points.clone();
    outline.push(points[0]);
    nodes.push(Node::Polygon { points, ink });
    nodes.push(line(outline, GLYPH_W, ink));
}

/// Previous, play or pause, next, centred under the ruler (mock `cx`, `ty`).
fn transport(facts: &MediaFacts, nodes: &mut Vec<Node>, targets: &mut Vec<HitTarget>) {
    let cx = RULER.x + RULER.w / 2.0;
    let ty = TRANSPORT_Y;
    let ink = |on: bool| if on { Ink::Ink } else { Ink::Faint };
    let skip = |dir: f64, on: bool, nodes: &mut Vec<Node>| {
        let bar = cx + dir * 50.0;
        let base = cx + dir * 38.0;
        nodes.push(line(
            vec![(bar, ty - 6.0), (bar, ty + 6.0)],
            GLYPH_W,
            ink(on),
        ));
        glyph(
            nodes,
            vec![(base, ty - 6.0), (cx + dir * 48.0, ty), (base, ty + 6.0)],
            ink(on),
        );
    };
    skip(-1.0, facts.can_prev, nodes);
    skip(1.0, facts.can_next, nodes);
    if facts.playing {
        for x in [cx - 5.0, cx + 3.0] {
            nodes.push(Node::Fill {
                rect: rect(x, ty - 7.0, 3.0, 14.0),
                radius: 0.0,
                ink: Ink::Ink,
            });
        }
    } else {
        glyph(
            nodes,
            vec![(cx - 5.0, ty - 7.0), (cx + 6.0, ty), (cx - 5.0, ty + 7.0)],
            Ink::Ink,
        );
    }
    let mut add = |x: f64, w: f64, action: Action| {
        targets.push(HitTarget {
            rect: rect(x, ty - 10.0, w, 20.0),
            action,
        });
    };
    if facts.can_prev {
        add(cx - 54.0, 20.0, Action::Previous);
    }
    add(cx - 12.0, 24.0, Action::PlayPause);
    if facts.can_next {
        add(cx + 34.0, 20.0, Action::Next);
    }
}

#[cfg(test)]
mod tests;
