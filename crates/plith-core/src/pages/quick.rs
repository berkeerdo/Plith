//! The quick page behind the gear mark: the six resting looks as small true previews, and one row
//! that opens the settings window. No tab, like the welcome view.

use super::kit::{line, rect, station, text};
use super::tabbar::tabbar;
use super::{Action, HitTarget, MARGIN_LEFT, PAGE_W, PageBuild, PageId};
use crate::draw::{Align, DisplayList, Ink, Node, Rect, StationMark, TextStyle};
use crate::lang::sheet::{SheetText, sheet_text};
use crate::lang::{Lang, Text, text as copy};
use crate::settings::IdleLook;
use crate::shapes;
use crate::sheet::preview::pill_preview;
use std::sync::Arc;

/// The looks in preview order, with their names.
const LOOKS: [(IdleLook, Text); 6] = [
    (IdleLook::Karot, Text::LookKarot),
    (IdleLook::Clock, Text::LookClock),
    (IdleLook::Playing, Text::LookPlaying),
    (IdleLook::Seal, Text::LookSeal),
    (IdleLook::Plain, Text::LookPlain),
    (IdleLook::Hidden, Text::LookHidden),
];

/// Scale of a preview inset: 200 x 32 becomes 80 x 12.8.
const PREVIEW_SCALE: f64 = 0.4;
/// Width of the row of previews.
const ROW_W: f64 = 560.0;
/// Gap between two preview cells.
const GAP: f64 = 12.0;
/// Width of a preview cell: six cells and five gaps fill the row.
const CELL_W: f64 = (ROW_W - 5.0 * GAP) / 6.0;
const CELL_Y: f64 = 80.0;
const CELL_H: f64 = 56.0;
/// Room of a look name, as budgeted in `lang::sheet::fit`.
const NAME_W: f64 = 70.0;
/// Y of the rule above the settings row.
const RULE_Y: f64 = 156.0;
const ROW_Y: f64 = 164.0;
const ROW_H: f64 = 36.0;
/// Room of the `all settings` word, as budgeted in `lang::sheet::fit`.
const WORD_W: f64 = 150.0;
/// Right end of the content column.
const RIGHT: f64 = MARGIN_LEFT + ROW_W;

/// Everything the quick page needs.
#[derive(Debug, Clone, PartialEq)]
pub struct QuickFacts {
    /// The look picked now.
    pub look: IdleLook,
    /// The resting pill of each look, in preview order (Karot, Clock, Playing, Seal, Plain,
    /// Hidden), built from the live facts.
    pub previews: [Arc<DisplayList>; 6],
    /// The interface language.
    pub lang: Lang,
}

/// The quick page.
#[must_use]
pub fn quick_page(f: &QuickFacts) -> PageBuild {
    let (mut nodes, mut targets) = tabbar(Some(PageId::Quick), "", f.lang);
    nodes.push(text(
        rect(MARGIN_LEFT, 56.0, ROW_W, 12.0),
        sheet_text(SheetText::CaptionClosed, f.lang),
        TextStyle::Label,
        Ink::Faint,
        Align::Start,
    ));
    for (i, (look, name)) in LOOKS.into_iter().enumerate() {
        let (cell_nodes, target) = cell(f, i, look, name);
        nodes.extend(cell_nodes);
        targets.push(target);
    }
    nodes.push(line(
        vec![(MARGIN_LEFT, RULE_Y), (RIGHT, RULE_Y)],
        1.0,
        Ink::Border,
    ));
    nodes.push(text(
        rect(MARGIN_LEFT, ROW_Y + 12.0, WORD_W, 12.0),
        sheet_text(SheetText::WordAllSettings, f.lang),
        TextStyle::Label,
        Ink::Ink,
        Align::Start,
    ));
    nodes.push(open_glyph(RIGHT - 16.0, ROW_Y + ROW_H / 2.0 - 5.0));
    targets.push(HitTarget {
        rect: rect(MARGIN_LEFT, ROW_Y, ROW_W, ROW_H),
        action: Action::OpenSettings,
    });
    PageBuild {
        list: DisplayList {
            nodes,
            size: (PAGE_W, shapes::QUICK.height),
        },
        targets,
        shape: shapes::QUICK,
    }
}

/// One look: its outline, the preview, the mark and name under it, and the click target.
fn cell(f: &QuickFacts, i: usize, look: IdleLook, name: Text) -> (Vec<Node>, HitTarget) {
    #[allow(clippy::cast_precision_loss)] // six cells
    let x = MARGIN_LEFT + i as f64 * (CELL_W + GAP);
    let on = f.look == look;
    let ink = if on { Ink::Ink } else { Ink::Faint };
    let area = Rect {
        x,
        y: CELL_Y,
        w: CELL_W,
        h: CELL_H,
    };
    let mut nodes = vec![outline(area, if on { Ink::Ink } else { Ink::Border })];
    nodes.extend(pill_preview(
        look,
        Arc::clone(&f.previews[i]),
        (x + (CELL_W - 200.0 * PREVIEW_SCALE) / 2.0, CELL_Y + 10.0),
        PREVIEW_SCALE,
    ));
    let mark = if on {
        StationMark::Filled
    } else {
        StationMark::Hollow
    };
    nodes.push(station(x + 6.0, CELL_Y + 40.0, mark, ink));
    nodes.push(text(
        rect(x + 13.0, CELL_Y + 34.0, NAME_W, 12.0),
        copy(name, f.lang),
        TextStyle::Label,
        ink,
        Align::Start,
    ));
    let target = HitTarget {
        rect: area,
        action: Action::Look(look),
    };
    (nodes, target)
}

/// A one pixel rectangle outline inside `r`.
fn outline(r: Rect, ink: Ink) -> Node {
    let (x0, y0, x1, y1) = (r.x + 0.5, r.y + 0.5, r.x + r.w - 0.5, r.y + r.h - 0.5);
    line(
        vec![(x0, y0), (x1, y0), (x1, y1), (x0, y1), (x0, y0)],
        1.0,
        ink,
    )
}

/// The open mark: an arrow leaving a corner, 10 DIP square with its origin at (`x`, `y`).
fn open_glyph(x: f64, y: f64) -> Node {
    line(
        vec![
            (x, y + 10.0),
            (x + 10.0, y),
            (x + 3.0, y),
            (x + 10.0, y),
            (x + 10.0, y + 7.0),
        ],
        1.5,
        Ink::Faint,
    )
}

#[cfg(test)]
mod tests;
