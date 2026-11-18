//! Routes and stations of the track diagram.

use super::{
    CUT_END_X, CUT_LABEL_X, DIAGONAL_END, LABEL_H, MIN_LEG, OUTPUT_X, SOURCE_X, source_has,
    station_y,
};
use crate::draw::{Align, Ink, Node, StationMark, TextStyle};
use crate::lang::{Lang, Text, text as word};
use crate::pages::audio::{AudioView, RouteState};
use crate::pages::kit::{rect, station, text};

#[cfg(test)]
mod tests;

/// Width of the `SESSİZ` label on a cut line.
pub(crate) const CUT_LABEL_W: f64 = 64.0;
/// Extra distance back for each shorter route converging on the same output.
const STAGGER: f64 = 30.0;
/// The candidate rail's offset from the picked source station.
const RAIL_DX: f64 = 8.0;
const RAIL_DY: f64 = 9.0;
/// Where a candidate's diagonal ends before the output station.
const RAIL_END: f64 = 12.0;
/// Two y values closer than this are the same row.
const EPS: f64 = 1e-6;

/// A route polyline: horizontal from the source station, a 45 degree run, horizontal into the
/// output station. Same height: two points. The run ends 22 DIP before the output station; a
/// taller rise first slides that end right, up to 6 DIP before the station. Only a rise above
/// the 188 DIP reach (six or more rows at pitch 40) cannot stay at 45 degrees: that documented
/// fallback keeps
/// 6 DIP legs at both ends and runs steeper.
#[must_use]
pub fn route_path(from: (f64, f64), to: (f64, f64)) -> Vec<(f64, f64)> {
    route_path_back(from, to, DIAGONAL_END)
}

/// [`route_path`] with the diagonal ending `back` DIP before the output station.
fn route_path_back(from: (f64, f64), to: (f64, f64), back: f64) -> Vec<(f64, f64)> {
    let dy = (to.1 - from.1).abs();
    if dy == 0.0 {
        return vec![from, to];
    }
    let mut end = to.0 - back;
    let mut start = end - dy;
    if start < from.0 + MIN_LEG {
        end = (from.0 + MIN_LEG + dy).min(to.0 - MIN_LEG);
        start = (end - dy).max(from.0 + MIN_LEG);
    }
    vec![from, (start, from.1), (end, to.1), to]
}

/// How far back each drawn route's diagonal ends, by route index. Routes that converge on one
/// output from the same side would share their last diagonal, so the tallest rise ends 22 DIP
/// before the station and each shorter one 30 DIP further back (mock: A1 at 430 and 400).
fn backs(view: &AudioView, slots: &[Option<usize>]) -> Vec<f64> {
    let mut backs = vec![DIAGONAL_END; view.routes.len()];
    // Each drawn route's output slot and rise (output minus source station y).
    let rises: Vec<Option<(usize, f64)>> = view
        .routes
        .iter()
        .map(|r| {
            let slot = (*slots.get(r.output)?)?;
            let stub = r.state == RouteState::Cut && view.sources.get(r.source)?.muted;
            let skip = stub || r.state == RouteState::Candidate;
            (!skip).then(|| (slot, station_y(view, slot) - station_y(view, r.source)))
        })
        .collect();
    let level = |a: f64, b: f64| (a - b).abs() < EPS;
    for (k, (back, rise)) in backs.iter_mut().zip(&rises).enumerate() {
        let Some((slot, dy)) = *rise else { continue };
        if level(dy, 0.0) {
            continue;
        }
        let taller =
            |m: usize, d: f64| d.abs() > dy.abs() + EPS || (level(d.abs(), dy.abs()) && m < k);
        let ahead = rises
            .iter()
            .enumerate()
            .filter(|(m, r)| {
                r.is_some_and(|(s, d)| {
                    s == slot && !level(d, 0.0) && (d > 0.0) == (dy > 0.0) && taller(*m, d)
                })
            })
            .count();
        #[allow(clippy::cast_precision_loss)]
        // Route counts are tiny, the cast is exact.
        let ahead = ahead as f64;
        *back = DIAGONAL_END + ahead * STAGGER;
    }
    backs
}

/// A candidate's dashed path on the rail below the picked source (mock lines 171 to 173): it
/// leaves at (`XS + 8`, `y + 9`), runs straight under the source's own row, or turns at 45
/// degrees to end 12 DIP before another output's station.
fn candidate_path(from: (f64, f64), to: (f64, f64)) -> Vec<(f64, f64)> {
    let rail = (from.0 + RAIL_DX, from.1 + RAIL_DY);
    if (to.1 - from.1).abs() < EPS {
        return vec![rail, (to.0, rail.1)];
    }
    route_path_back(rail, to, RAIL_END)
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
/// drawn as the stub; a cut caused by a muted output keeps its full path in track ink. Routes to
/// hidden outputs (no slot) are not drawn.
pub(super) fn draw(view: &AudioView, slots: &[Option<usize>], lang: Lang, nodes: &mut Vec<Node>) {
    let backs = backs(view, slots);
    for order in 0..3 {
        for (k, r) in view.routes.iter().enumerate() {
            let Some(source) = view.sources.get(r.source) else {
                continue;
            };
            let (at, width, ink, dashed) = look(r.state);
            let stub = r.state == RouteState::Cut && source.muted;
            let Some(Some(slot)) = slots.get(r.output) else {
                continue;
            };
            if at != order || stub {
                continue;
            }
            let from = (SOURCE_X, station_y(view, r.source));
            let to = (OUTPUT_X, station_y(view, *slot));
            let path = if r.state == RouteState::Candidate {
                candidate_path(from, to)
            } else {
                route_path_back(from, to, backs[k])
            };
            nodes.push(stroke(path, width, ink, dashed));
        }
    }
    for (s, row) in view.sources.iter().enumerate() {
        if row.muted && source_has(view, s, RouteState::Cut) {
            cut(station_y(view, s), lang, nodes);
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
        let Some(slot) = slots[o] else {
            continue;
        };
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
        nodes.push(station(OUTPUT_X, station_y(view, slot), mark, ink));
    }
}

/// The mock's cut line: a stub from the source station to a hollow station, then the muted word.
fn cut(y: f64, lang: Lang, nodes: &mut Vec<Node>) {
    nodes.push(stroke(
        vec![(SOURCE_X, y), (CUT_END_X, y)],
        1.0,
        Ink::Track,
        false,
    ));
    nodes.push(station(CUT_END_X, y, StationMark::Hollow, Ink::Faint));
    nodes.push(text(
        rect(CUT_LABEL_X, y + 4.0 - 10.0, CUT_LABEL_W, LABEL_H),
        word(Text::Muted, lang),
        TextStyle::Label,
        Ink::Faint,
        Align::Start,
    ));
}
