//! The tab bar: one hairline track with four stations and a clock at the right end.

use super::kit::{rect, text};
use super::{Action, HitTarget, MARGIN_LEFT, MARGIN_RIGHT, PAGE_W, PageId};
use crate::draw::{Align, Ink, Node, Rect, StationMark, TextStyle};

/// Centre x of the first station.
const FIRST_X: f64 = MARGIN_LEFT;
/// Distance between station centres.
const STEP: f64 = 112.0;
/// Distance from a station centre to its label and hit area left edge.
const LABEL_DX: f64 = 3.0;
/// Y of the track and the station centres.
const LINE_Y: f64 = 34.0;
/// Top of the labels (mock baseline 22, mono 10).
const LABEL_Y: f64 = 12.0;
const LABEL_H: f64 = 12.0;
const LABEL_W: f64 = STEP - 16.0;
const CLOCK_W: f64 = 80.0;
/// Clock box height: its top lands on the mock baseline 38 minus the mono 12 size.
const CLOCK_H: f64 = 16.0;
/// How far the hit area reaches below the line.
const HIT_BELOW: f64 = 6.0;
/// Gap between neighbouring hit areas.
const HIT_GAP: f64 = 8.0;
/// Reach of the last hit area to the right of its station.
const LAST_REACH: f64 = 80.0;

/// The clock text, both parts zero-padded (`09:05`).
#[must_use]
pub fn clock_text(hour: u8, minute: u8) -> String {
    format!("{hour:02}:{minute:02}")
}

/// The tab bar nodes and one hit target per tab. With `active` of `None` no station is filled, and
/// an empty `clock` draws no clock text.
#[must_use]
pub fn tabbar(active: Option<PageId>, clock: &str) -> (Vec<Node>, Vec<HitTarget>) {
    let last_x = FIRST_X + STEP * 3.0;
    let mut nodes = vec![Node::Stroke {
        points: vec![(FIRST_X, LINE_Y), (last_x, LINE_Y)],
        width: 1.0,
        ink: Ink::FaintTrack,
        dashed: false,
    }];
    let mut targets = Vec::with_capacity(PageId::TABS.len());
    for (page, slot) in PageId::TABS.into_iter().zip([0.0, 1.0, 2.0, 3.0]) {
        let x = FIRST_X + STEP * slot;
        let on = active == Some(page);
        let ink = if on { Ink::Ink } else { Ink::Faint };
        let left = x - LABEL_DX;
        nodes.push(Node::Station {
            x,
            y: LINE_Y,
            mark: if on {
                StationMark::Filled
            } else {
                StationMark::Hollow
            },
            ink,
        });
        nodes.push(text(
            rect(left, LABEL_Y, LABEL_W, LABEL_H),
            page.label(),
            TextStyle::Label,
            ink,
            Align::Start,
        ));
        let right = if page == PageId::System {
            x + LAST_REACH
        } else {
            x + STEP - LABEL_DX - HIT_GAP
        };
        targets.push(HitTarget {
            rect: Rect {
                x: left,
                y: LABEL_Y,
                w: right - left,
                h: LINE_Y + HIT_BELOW - LABEL_Y,
            },
            action: Action::Tab(page),
        });
    }
    let right_edge = PAGE_W - MARGIN_RIGHT;
    if !clock.is_empty() {
        nodes.push(text(
            rect(
                right_edge - CLOCK_W,
                LINE_Y - CLOCK_H / 2.0,
                CLOCK_W,
                CLOCK_H,
            ),
            clock,
            TextStyle::Clock,
            Ink::Muted,
            Align::End,
        ));
    }
    (nodes, targets)
}

#[cfg(test)]
mod tests;
