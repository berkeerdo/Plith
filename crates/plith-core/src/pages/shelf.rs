//! The shelf page, empty state only (dropping files is R5).
//!
//! Positions come from `shelfEmpty()` in `docs/design/r3/ray-paneli.html`. The mock island is
//! 170 high and so is the empty page (the spec's 214 is the R5 state with wagons).

use super::kit::{line, rect, station, text};
use super::tabbar::tabbar;
use super::{PAGE_W, PageBuild, PageId};
use crate::draw::{Align, DisplayList, Ink, Node, StationMark, TextStyle};
use crate::shapes;

/// Empty page height (mock `shelfEmpty`, `h = 170`).
const HEIGHT: f64 = 170.0;
/// The dashed drop box (mock `rect x 30, y 58, W - 60, 70`).
const BOX_X: f64 = 30.0;
const BOX_Y: f64 = 58.0;
const BOX_H: f64 = 70.0;
/// The siding line y (mock 146).
const SIDING_Y: f64 = 146.0;

/// The empty shelf: the drop box with its copy and the siding line.
#[must_use]
pub fn shelf_page(clock: &str) -> PageBuild {
    let (mut nodes, targets) = tabbar(PageId::Shelf, clock);
    let right = PAGE_W - 30.0;
    nodes.push(Node::Stroke {
        points: vec![
            (BOX_X, BOX_Y),
            (right, BOX_Y),
            (right, BOX_Y + BOX_H),
            (BOX_X, BOX_Y + BOX_H),
            (BOX_X, BOX_Y),
        ],
        width: 1.0,
        ink: Ink::Faint,
        dashed: true,
    });
    nodes.push(text(
        rect(BOX_X, 78.0, right - BOX_X, 16.0),
        "Dosyalar\u{131} buraya s\u{fc}r\u{fc}kle",
        TextStyle::Name,
        Ink::Ink,
        Align::Center,
    ));
    nodes.push(text(
        rect(BOX_X, 98.0, right - BOX_X, 12.0),
        "RAFTA BEKLER, \u{130}STED\u{130}\u{11e}\u{130}N YERE GER\u{130} S\u{dc}R\u{dc}KLERS\u{130}N",
        TextStyle::Label,
        Ink::Faint,
        Align::Center,
    ));
    nodes.push(line(
        vec![(BOX_X, SIDING_Y), (right, SIDING_Y)],
        1.0,
        Ink::Track,
    ));
    nodes.push(station(BOX_X, SIDING_Y, StationMark::Hollow, Ink::Faint));
    nodes.push(station(right, SIDING_Y, StationMark::Hollow, Ink::Faint));
    let mut shape = shapes::PAGES[PageId::Shelf.index()];
    shape.width = PAGE_W;
    shape.height = HEIGHT;
    PageBuild {
        list: DisplayList {
            nodes,
            size: (PAGE_W, HEIGHT),
        },
        targets,
        shape,
    }
}

#[cfg(test)]
mod tests;
