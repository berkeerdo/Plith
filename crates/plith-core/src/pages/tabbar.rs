//! The tab bar: one hairline track with four stations and a clock at the right end.

use std::sync::Arc;

use super::{Action, HitTarget, MARGIN_LEFT, MARGIN_RIGHT, PAGE_W, PageId};
use crate::draw::{Align, Ink, Node, Rect, StationMark, TextStyle, Trim};

/// Centre x of the first station.
const FIRST_X: f64 = MARGIN_LEFT;
/// Distance between station centres.
const STEP: f64 = 112.0;
/// Station square size.
const STATION: f64 = 7.0;
/// Y of the track and the station centres.
const LINE_Y: f64 = 34.0;
/// Top of the labels.
const LABEL_Y: f64 = 14.0;
const LABEL_H: f64 = 12.0;
const LABEL_W: f64 = STEP - 16.0;
const CLOCK_W: f64 = 80.0;
const CLOCK_H: f64 = 18.0;
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

/// The tab bar nodes and one hit target per tab.
#[must_use]
pub fn tabbar(active: PageId, clock: &str) -> (Vec<Node>, Vec<HitTarget>) {
    let last_x = FIRST_X + STEP * 3.0;
    let mut nodes = vec![Node::Stroke {
        points: vec![(FIRST_X, LINE_Y), (last_x, LINE_Y)],
        width: 1.0,
        ink: Ink::Track,
        dashed: false,
    }];
    let mut targets = Vec::with_capacity(4);
    for (page, slot) in PageId::ALL.into_iter().zip([0.0, 1.0, 2.0, 3.0]) {
        let x = FIRST_X + STEP * slot;
        let on = page == active;
        let ink = if on { Ink::Ink } else { Ink::Faint };
        let left = x - STATION / 2.0;
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
        nodes.push(Node::Text {
            rect: Rect {
                x: left,
                y: LABEL_Y,
                w: LABEL_W,
                h: LABEL_H,
            },
            text: Arc::from(page.label()),
            style: TextStyle::Label,
            ink,
            align: Align::Start,
            trim: Trim::End,
        });
        let right = if page == PageId::System {
            x + LAST_REACH
        } else {
            x + STEP - STATION / 2.0 - HIT_GAP
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
    nodes.push(Node::Text {
        rect: Rect {
            x: right_edge - CLOCK_W,
            y: LINE_Y - CLOCK_H / 2.0,
            w: CLOCK_W,
            h: CLOCK_H,
        },
        text: Arc::from(clock),
        style: TextStyle::Clock,
        ink: Ink::Ink,
        align: Align::End,
        trim: Trim::End,
    });
    (nodes, targets)
}

#[cfg(test)]
mod tests;
