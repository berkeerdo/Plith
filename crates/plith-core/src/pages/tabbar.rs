//! The tab bar: a home mark, one hairline track with four stations, a clock and a gear mark.

use super::glyphs::{GLYPH, gear, home};
use super::kit::{rect, text};
use super::{Action, HitTarget, MARGIN_LEFT, MARGIN_RIGHT, PAGE_W, PageId};
use crate::draw::{Align, Ink, Node, Rect, StationMark, TextStyle};
use crate::lang::Lang;

/// Centre x and y of the home mark (y is the middle of the label and line band).
const HOME_X: f64 = MARGIN_LEFT;
const HOME_Y: f64 = 23.0;
/// Centre x of the gear mark, mirroring the home mark at the right end.
const GEAR_X: f64 = 590.0;
/// Half width of the home hit area minus its centre pixel.
const HOME_REACH_X: f64 = 12.0;
/// Centre x of the first station, one slot right of the home mark.
const FIRST_X: f64 = MARGIN_LEFT + 24.0;
/// Distance between station centres.
const STEP: f64 = 112.0;
/// Distance from a station centre to its label and hit area left edge.
const LABEL_DX: f64 = 3.0;
/// Y of the track and the station centres.
const LINE_Y: f64 = 34.0;
/// Top of the labels (mock baseline 22, mono 10).
const LABEL_Y: f64 = 12.0;
const LABEL_H: f64 = 12.0;
pub(crate) const LABEL_W: f64 = STEP - 16.0;
const CLOCK_W: f64 = 80.0;
/// How far the clock sits left of the right margin: room for the gear and a clear gap, so the
/// gear does not read as the clock's own control (Berke 2026-11-24).
const CLOCK_SHIFT: f64 = 32.0;
/// Clock box height. The box is centred on the home and gear marks' band (`HOME_Y`), so the
/// clock and the gear sit on one row.
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

/// The tab bar nodes and a hit target for the home mark, each tab and the gear mark. With `active` of `None` no
/// station is filled; `Some(PageId::Welcome)` inks the home mark instead. An empty `clock` draws
/// no clock text. The labels read in `lang`.
#[must_use]
pub fn tabbar(active: Option<PageId>, clock: &str, lang: Lang) -> (Vec<Node>, Vec<HitTarget>) {
    let last_x = FIRST_X + STEP * 3.0;
    let mut nodes = vec![Node::Stroke {
        points: vec![(FIRST_X, LINE_Y), (last_x, LINE_Y)],
        width: 1.0,
        ink: Ink::FaintTrack,
        dashed: false,
    }];
    let home_ink = if active == Some(PageId::Welcome) {
        Ink::Ink
    } else {
        Ink::Faint
    };
    nodes.extend(home(HOME_X - GLYPH / 2.0, HOME_Y - GLYPH / 2.0, home_ink));
    let mut targets = Vec::with_capacity(PageId::TABS.len() + 2);
    // First in the list: `hit` returns the first target under a point, and the home area's slack
    // reaches the first station's label.
    targets.push(HitTarget {
        rect: Rect {
            x: HOME_X - HOME_REACH_X,
            y: LABEL_Y,
            w: HOME_REACH_X * 2.0 + 1.0,
            h: LINE_Y + HIT_BELOW - LABEL_Y,
        },
        action: Action::Tab(PageId::Welcome),
    });
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
            page.label(lang),
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
    let right_edge = PAGE_W - MARGIN_RIGHT - CLOCK_SHIFT;
    if !clock.is_empty() {
        nodes.push(text(
            rect(
                right_edge - CLOCK_W,
                HOME_Y - CLOCK_H / 2.0,
                CLOCK_W,
                CLOCK_H,
            ),
            clock,
            TextStyle::Clock,
            Ink::Muted,
            Align::End,
        ));
    }
    // Last in the list: it sits right of the clock and overlaps nothing.
    let (gear_nodes, gear_target) = gear_mark(active == Some(PageId::Quick));
    nodes.extend(gear_nodes);
    targets.push(gear_target);
    (nodes, targets)
}

/// The gear mark and its target. Open, the gear draws in full ink and is the way back to the
/// welcome view, like the home mark.
fn gear_mark(on_quick: bool) -> (Vec<Node>, HitTarget) {
    let ink = if on_quick { Ink::Ink } else { Ink::Faint };
    let nodes = gear(GEAR_X - GLYPH / 2.0, HOME_Y - GLYPH / 2.0, ink);
    let target = HitTarget {
        rect: Rect {
            x: GEAR_X - HOME_REACH_X,
            y: LABEL_Y,
            w: HOME_REACH_X * 2.0 + 1.0,
            h: LINE_Y + HIT_BELOW - LABEL_Y,
        },
        action: Action::Tab(if on_quick {
            PageId::Welcome
        } else {
            PageId::Quick
        }),
    };
    (nodes, target)
}

#[cfg(test)]
mod tests;
