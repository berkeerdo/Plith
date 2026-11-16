//! Routes and stations of the track diagram.

use std::sync::Arc;

use super::{
    CUT_END_X, CUT_LABEL_X, LABEL_H, LABEL_TEXT, OUTPUT_X, SOURCE_X, source_has, station_y,
};
use crate::draw::{Align, Ink, Node, Rect, StationMark, TextStyle, Trim};
use crate::pages::audio::{AudioView, RouteState};

/// Where a diagonal ends before the output station (mock: the Discord to A1 rise runs from
/// x 280 to 430 with `XO = 452`).
const DIAGONAL_END: f64 = 22.0;
/// Shortest horizontal leg when a rise is too tall for a 45 degree run.
const MIN_LEG: f64 = 6.0;
/// Width of the `SESSİZ` label on a cut line.
const CUT_LABEL_W: f64 = 64.0;

/// A route polyline: horizontal from the source station, a 45 degree run that ends 22 DIP before
/// the output station, horizontal into it. Same height: two points. A rise taller than the
/// column gap allows keeps short legs and runs steeper than 45 degrees.
#[must_use]
pub fn route_path(from: (f64, f64), to: (f64, f64)) -> Vec<(f64, f64)> {
    let dy = (to.1 - from.1).abs();
    if dy == 0.0 {
        return vec![from, to];
    }
    let mut end = to.0 - DIAGONAL_END;
    let mut start = end - dy;
    if start < from.0 + MIN_LEG {
        start = from.0 + MIN_LEG;
        end = end.max(start).min(to.0 - MIN_LEG);
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

fn station(x: f64, y: f64, mark: StationMark, ink: Ink) -> Node {
    Node::Station { x, y, mark, ink }
}

/// Idle, candidate and live routes (in that order, lit on top), cut stubs, then stations.
pub(super) fn draw(view: &AudioView, nodes: &mut Vec<Node>) {
    let order = [
        (RouteState::Idle, 1.0, Ink::Track, false),
        (RouteState::Candidate, 1.0, Ink::Flow, true),
        (RouteState::Live, 2.0, Ink::Flow, false),
    ];
    for (state, width, ink, dashed) in order {
        for r in view.routes.iter().filter(|r| r.state == state) {
            if r.source >= view.sources.len() || r.output >= view.outputs.len() {
                continue;
            }
            let from = (SOURCE_X, station_y(view, r.source));
            let to = (OUTPUT_X, station_y(view, r.output));
            nodes.push(stroke(route_path(from, to), width, ink, dashed));
        }
    }
    for s in 0..view.sources.len() {
        if source_has(view, s, RouteState::Cut) {
            cut(station_y(view, s), nodes);
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

/// The mock's cut line: a stub from the source station to a hollow station, then `SESSİZ`.
fn cut(y: f64, nodes: &mut Vec<Node>) {
    nodes.push(stroke(
        vec![(SOURCE_X, y), (CUT_END_X, y)],
        1.0,
        Ink::Track,
        false,
    ));
    nodes.push(station(CUT_END_X, y, StationMark::Hollow, Ink::Faint));
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
