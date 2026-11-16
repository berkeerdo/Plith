//! The rest pill: one seal with three fixed slots (output name, route, value).
//!
//! Positions come from `seal()` in `docs/design/r3/ray-paneli.html` (`w = 200`, `h = 32`,
//! `outW = 52`). The name and value rects are the same in every state; only the route changes.

use super::kit::{line, rect, station, text};
use crate::draw::{Align, DisplayList, Ink, Node, StationMark, TextStyle};
use crate::shapes;

/// What the pill shows.
#[derive(Debug, Clone, PartialEq)]
pub struct PillFacts {
    /// The short output name (for example `G733`).
    pub output: String,
    /// The volume percentage, when known.
    pub value: Option<u8>,
    /// The state of the route.
    pub state: PillState,
}

/// The three looks of the pill route.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PillState {
    /// Sound flows to the output.
    Live,
    /// The output is muted: the route is cut and the value slot reads `KAPALI`.
    Muted,
    /// Nothing plays.
    Idle,
}

/// Left padding of the name (mock `16`).
const PAD_LEFT: f64 = 16.0;
/// Right padding of the value (mock `w - 14`).
const PAD_RIGHT: f64 = 14.0;
/// Width of the name slot (mock `outW = 52`).
const NAME_W: f64 = 52.0;
/// Width of the value slot: wide enough for `KAPALI`.
const VALUE_W: f64 = 48.0;
/// Route end x offset from the right edge: 64, or 76 when muted (mock `xb`).
const ROUTE_END: f64 = 64.0;
const ROUTE_END_MUTED: f64 = 76.0;
/// The cut gap of the muted route, as fractions of the route (mock 0.4 and 0.6).
const CUT_FROM: f64 = 0.4;
const CUT_TO: f64 = 0.6;

/// The pill display list, 200 x 32.
#[must_use]
pub fn pill(facts: &PillFacts) -> DisplayList {
    let (w, h) = (shapes::REST.width, shapes::REST.height);
    let y = h / 2.0;
    let muted = facts.state == PillState::Muted;
    let mut nodes = vec![text(
        rect(PAD_LEFT, y - 6.0, NAME_W - 4.0, 12.0),
        &facts.output,
        TextStyle::Label,
        Ink::Ink,
        Align::Start,
    )];
    let xa = PAD_LEFT + NAME_W;
    let xb = w - if muted { ROUTE_END_MUTED } else { ROUTE_END };
    route(&mut nodes, facts.state, xa, xb, y);
    let value_rect = rect(w - PAD_RIGHT - VALUE_W, y - 6.0, VALUE_W, 12.0);
    if muted {
        nodes.push(text(
            value_rect,
            "KAPALI",
            TextStyle::Label,
            Ink::Faint,
            Align::End,
        ));
    } else if let Some(value) = facts.value {
        nodes.push(text(
            value_rect,
            &value.to_string(),
            TextStyle::Label,
            Ink::Ink,
            Align::End,
        ));
    }
    DisplayList {
        nodes,
        size: (w, h),
    }
}

fn route(nodes: &mut Vec<Node>, state: PillState, xa: f64, xb: f64, y: f64) {
    match state {
        PillState::Live => {
            nodes.push(line(vec![(xa, y), (xb, y)], 2.0, Ink::Flow));
            nodes.push(station(xa, y, StationMark::Filled, Ink::Flow));
            nodes.push(station(xb, y, StationMark::Filled, Ink::Flow));
        }
        PillState::Muted => {
            let span = xb - xa;
            nodes.push(line(
                vec![(xa, y), (xa + span * CUT_FROM, y)],
                1.0,
                Ink::Track,
            ));
            nodes.push(line(
                vec![(xa + span * CUT_TO, y), (xb, y)],
                1.0,
                Ink::Track,
            ));
            nodes.push(station(xa, y, StationMark::Filled, Ink::Ink));
            nodes.push(station(xb, y, StationMark::Hollow, Ink::Faint));
        }
        PillState::Idle => {
            nodes.push(line(vec![(xa, y), (xb, y)], 1.0, Ink::Track));
            nodes.push(station(xa, y, StationMark::Hollow, Ink::Faint));
            nodes.push(station(xb, y, StationMark::Hollow, Ink::Faint));
        }
    }
}

#[cfg(test)]
mod tests;
