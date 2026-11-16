//! Routes and stations of the track diagram.

use std::sync::Arc;

use super::{
    CUT_END_X, CUT_LABEL_X, DIAGONAL_END, LABEL_H, LABEL_TEXT, MIN_LEG, OUTPUT_X, SOURCE_X,
    has_voicemeeter, source_has, station_y,
};
use crate::draw::{Align, Ink, Node, Rect, StationMark, TextStyle, Trim};
use crate::pages::audio::{AudioView, RouteState};
use crate::pages::kit::station;

/// Width of the `SESSİZ` label on a cut line.
const CUT_LABEL_W: f64 = 64.0;

/// A route polyline: horizontal from the source station, a 45 degree run, horizontal into the
/// output station. Same height: two points. The run ends 22 DIP before the output station; a
/// taller rise first slides that end right, up to 6 DIP before the station. Only a rise above
/// the 188 DIP reach (Potato density) cannot stay at 45 degrees: that documented fallback keeps
/// 6 DIP legs at both ends and runs steeper.
#[must_use]
pub fn route_path(from: (f64, f64), to: (f64, f64)) -> Vec<(f64, f64)> {
    let dy = (to.1 - from.1).abs();
    if dy == 0.0 {
        return vec![from, to];
    }
    let mut end = to.0 - DIAGONAL_END;
    let mut start = end - dy;
    if start < from.0 + MIN_LEG {
        end = (from.0 + MIN_LEG + dy).min(to.0 - MIN_LEG);
        start = (end - dy).max(from.0 + MIN_LEG);
    }
    vec![from, (start, from.1), (end, to.1), to]
}

fn stroke(points: Vec<(f64, f64)>, width: f64, ink: Ink, dashed: bool) -> Node {
    Node::Stroke {
        points,
        width,
        ink,
        dashed,
    }
}

/// The drawing pass of a route state and its line: idle routes and bus-muted cuts first (1 px
/// track), candidates (dashed flow), live routes on top (2 px flow).
fn look(state: RouteState) -> (u8, f64, Ink, bool) {
    match state {
        RouteState::Idle | RouteState::Cut => (0, 1.0, Ink::Track, false),
        RouteState::Candidate => (1, 1.0, Ink::Flow, true),
        RouteState::Live => (2, 2.0, Ink::Flow, false),
    }
}

/// Routes by pass, the cut stubs of muted sources, then stations. A cut whose source is muted is
/// drawn as the stub; a cut caused by a muted output keeps its full path in track ink.
pub(super) fn draw(view: &AudioView, nodes: &mut Vec<Node>) {
    for order in 0..3 {
        for r in &view.routes {
            let Some(source) = view.sources.get(r.source) else {
                continue;
            };
            let (at, width, ink, dashed) = look(r.state);
            let stub = r.state == RouteState::Cut && source.muted;
            if at != order || stub || r.output >= view.outputs.len() {
                continue;
            }
            let from = (SOURCE_X, station_y(view, r.source));
            let to = (OUTPUT_X, station_y(view, r.output));
            nodes.push(stroke(route_path(from, to), width, ink, dashed));
        }
    }
    for (s, row) in view.sources.iter().enumerate() {
        if row.muted && source_has(view, s, RouteState::Cut) {
            cut(station_y(view, s), has_voicemeeter(view), nodes);
        }
    }
    for (s, row) in view.sources.iter().enumerate() {
        let (mark, ink) = if row.muted {
            (StationMark::Filled, Ink::Ink)
        } else if source_has(view, s, RouteState::Live) {
            (StationMark::Filled, Ink::Flow)
        } else if row.picked {
            (StationMark::Filled, Ink::Ink)
        } else {
            (StationMark::Hollow, Ink::Faint)
        };
        nodes.push(station(SOURCE_X, station_y(view, s), mark, ink));
    }
    for (o, row) in view.outputs.iter().enumerate() {
        let candidate = view
            .routes
            .iter()
            .any(|r| r.output == o && r.state == RouteState::Candidate);
        let (mark, ink) = if row.live {
            (StationMark::Filled, Ink::Flow)
        } else if row.muted {
            (StationMark::Filled, Ink::Ink)
        } else if candidate {
            (StationMark::Hollow, Ink::Flow)
        } else {
            (StationMark::Hollow, Ink::Faint)
        };
        nodes.push(station(OUTPUT_X, station_y(view, o), mark, ink));
    }
}

/// The mock's cut line: a stub from the source station to a hollow station, then `SESSİZ`
/// (without Voicemeeter the muted default output already says it, so no label).
fn cut(y: f64, label: bool, nodes: &mut Vec<Node>) {
    nodes.push(stroke(
        vec![(SOURCE_X, y), (CUT_END_X, y)],
        1.0,
        Ink::Track,
        false,
    ));
    nodes.push(station(CUT_END_X, y, StationMark::Hollow, Ink::Faint));
    if !label {
        return;
    }
    nodes.push(Node::Text {
        rect: Rect {
            x: CUT_LABEL_X,
            y: y + 4.0 - 10.0,
            w: CUT_LABEL_W,
            h: LABEL_H,
        },
        text: Arc::from(LABEL_TEXT),
        style: TextStyle::Label,
        ink: Ink::Faint,
        align: Align::Start,
        trim: Trim::End,
    });
}
