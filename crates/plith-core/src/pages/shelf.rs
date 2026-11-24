//! The shelf page: the empty drop box, or the wagons on their tracks with the drop slot.
//!
//! Positions come from `shelfEmpty()` and `shelfPage()` in `docs/design/r3/ray-paneli.html`
//! (the wagon layout lives in `shelf::layout`). The mock island is 170 high when empty, 214 with
//! one row of wagons, and one row pitch more for each further row.

mod wagons;

use super::kit::{line, rect, station, text};
use super::tabbar::tabbar;
use super::{Action, HitTarget, PAGE_W, PageBuild, PageId};
use crate::draw::{Align, DisplayList, Ink, Node, StationMark, TextStyle};
use crate::lang::{Lang, Text, text as word};
use crate::shapes;
use crate::shelf::ShelfItem;
use crate::shelf::layout::{UNDO_W, UNDO_X, layout};

/// The dashed drop box of the empty page (mock `rect x 30, y 58, W - 60, 70`).
const BOX_X: f64 = 30.0;
const BOX_Y: f64 = 58.0;
const BOX_H: f64 = 70.0;
/// The siding line y of the empty page (mock 146).
const SIDING_Y: f64 = 146.0;
/// Where the undo word sits on the empty page, which has no footer.
const EMPTY_UNDO_Y: f64 = 164.0;
/// Extra page height of the empty shelf while an undo is offered.
const EMPTY_UNDO_ROOM: f64 = 28.0;

/// What the shelf page shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ShelfView<'a> {
    /// The items, newest first.
    pub items: &'a [ShelfItem],
    /// A file drag hovers over the island: the slot is drawn hot.
    pub hot: bool,
    /// A removal can be taken back.
    pub can_undo: bool,
}

/// The empty shelf: the drop box with its copy and the siding line.
#[must_use]
pub fn shelf_page(clock: &str, lang: Lang) -> PageBuild {
    shelf_page_with(&ShelfView::default(), clock, lang)
}

/// The shelf page for `view`.
#[must_use]
pub fn shelf_page_with(view: &ShelfView, clock: &str, lang: Lang) -> PageBuild {
    let (mut nodes, tab_targets) = tabbar(Some(PageId::Shelf), clock, lang);
    let plan = layout(view.items.len());
    let mut height = plan.height;
    let mut targets = Vec::new();
    if view.items.is_empty() {
        empty_nodes(&mut nodes, view, lang);
        if view.can_undo {
            // The undo word gets its own band below the siding line.
            height += EMPTY_UNDO_ROOM;
            targets.push(undo_target(EMPTY_UNDO_Y));
        }
    } else {
        wagons::push(&mut nodes, &mut targets, view, &plan, lang);
    }
    targets.extend(tab_targets);
    let mut shape = shapes::PAGES[PageId::Shelf.index()];
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

/// The click target of the undo word, whose text box top is `y`.
fn undo_target(y: f64) -> HitTarget {
    HitTarget {
        rect: rect(UNDO_X, y, UNDO_W, 14.0),
        action: Action::ShelfUndo,
    }
}

/// The empty page: the drop box (hot while a drag hovers), its copy and the siding line.
fn empty_nodes(nodes: &mut Vec<Node>, view: &ShelfView, lang: Lang) {
    let right = PAGE_W - 30.0;
    let (edge, copy) = if view.hot {
        (Ink::Flow, Ink::Flow)
    } else {
        (Ink::Faint, Ink::Ink)
    };
    nodes.push(Node::Stroke {
        points: vec![
            (BOX_X, BOX_Y),
            (right, BOX_Y),
            (right, BOX_Y + BOX_H),
            (BOX_X, BOX_Y + BOX_H),
            (BOX_X, BOX_Y),
        ],
        width: 1.0,
        ink: edge,
        dashed: true,
    });
    nodes.push(text(
        rect(BOX_X, 78.0, right - BOX_X, 16.0),
        word(Text::ShelfDrop, lang),
        TextStyle::Name,
        copy,
        Align::Center,
    ));
    nodes.push(text(
        rect(BOX_X, 98.0, right - BOX_X, 12.0),
        word(Text::ShelfHint, lang),
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
    if view.can_undo {
        nodes.push(text(
            rect(UNDO_X, EMPTY_UNDO_Y, UNDO_W, 12.0),
            word(Text::ShelfUndo, lang),
            TextStyle::Label,
            Ink::Muted,
            Align::End,
        ));
    }
}

#[cfg(test)]
mod tests;
