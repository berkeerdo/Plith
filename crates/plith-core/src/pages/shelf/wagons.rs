//! The wagons of the shelf page: the outline, the type tag, the two-line name, two stations on
//! the track, the drop slot after the last wagon and the footer.

use super::{ShelfView, undo_target};
use crate::draw::{Align, Ink, Node, Rect, StationMark, TextStyle, Trim};
use crate::lang::{Lang, Text, files, text as word};
use crate::pages::kit::{line, rect, station, text, text_trim};
use crate::pages::{Action, HitTarget};
use crate::shelf::kind::{display_name, type_tag};
use crate::shelf::layout::{
    CLEAR_W, CLEAR_X, CROSS_SIZE, FOOTER_X, LEFT, Layout, RIGHT, TAG_W, UNDO_W, UNDO_X,
};
use crate::shelf::name::name_lines;

/// Text left inset inside a wagon and its width (a 104 wide wagon, 10 in from each side).
const INSET: f64 = 10.0;
/// Top of the type tag text box below the wagon top (mock baseline 80, wagon top 62).
const TAG_DY: f64 = 8.0;
/// Top of the first and second name line below the wagon top (mock baselines 124 and 140).
const NAME_DY: [f64; 2] = [50.0, 66.0];
/// Top of the slot label below the slot top (mock baseline 112).
const SLOT_LABEL_DY: f64 = 40.0;

/// A closed rectangle as a stroke.
fn outline(r: Rect, ink: Ink, dashed: bool) -> Node {
    Node::Stroke {
        points: vec![
            (r.x, r.y),
            (r.x + r.w, r.y),
            (r.x + r.w, r.y + r.h),
            (r.x, r.y + r.h),
            (r.x, r.y),
        ],
        width: 1.0,
        ink,
        dashed,
    }
}

/// Adds the tracks, wagons, slot and footer of `view` as laid out by `plan`.
pub(super) fn push(
    nodes: &mut Vec<Node>,
    targets: &mut Vec<HitTarget>,
    view: &ShelfView,
    plan: &Layout,
    lang: Lang,
) {
    for &y in &plan.tracks {
        nodes.push(line(vec![(LEFT, y), (RIGHT, y)], 1.0, Ink::Track));
    }
    for (index, (item, wagon)) in view.items.iter().zip(&plan.wagons).enumerate() {
        let r = wagon.rect;
        nodes.push(outline(r, Ink::Track, false));
        nodes.push(text(
            rect(r.x + INSET, r.y + TAG_DY, TAG_W, 12.0),
            &type_tag(item, lang),
            TextStyle::Label,
            Ink::Faint,
            Align::Start,
        ));
        let (first, rest) = name_lines(&display_name(item));
        for (i, (line_text, ink, trim)) in [
            (first, Ink::Ink, Trim::End),
            (rest, Ink::Muted, Trim::Middle),
        ]
        .into_iter()
        .enumerate()
        {
            if line_text.is_empty() {
                continue;
            }
            nodes.push(text_trim(
                rect(r.x + INSET, r.y + NAME_DY[i], r.w - 2.0 * INSET, 16.0),
                &line_text,
                TextStyle::Body,
                ink,
                Align::Start,
                trim,
            ));
        }
        for x in wagon.stations {
            nodes.push(station(x, wagon.track_y, StationMark::Filled, Ink::Ink));
        }
        push_cross(nodes, wagon.cross);
        targets.push(HitTarget {
            rect: wagon.remove,
            action: Action::ShelfRemove(index),
        });
    }
    for (index, wagon) in plan.wagons.iter().enumerate() {
        targets.push(HitTarget {
            rect: wagon.rect,
            action: Action::ShelfWagon(index),
        });
    }
    if let Some(slot) = plan.slot {
        push_slot(nodes, view.hot, slot, plan, lang);
    }
    push_footer(nodes, targets, view, plan, lang);
}

/// The remove mark: two thin diagonals centred on `(cx, cy)`.
fn push_cross(nodes: &mut Vec<Node>, (cx, cy): (f64, f64)) {
    let h = CROSS_SIZE / 2.0;
    nodes.push(line(
        vec![(cx - h, cy - h), (cx + h, cy + h)],
        1.0,
        Ink::Faint,
    ));
    nodes.push(line(
        vec![(cx - h, cy + h), (cx + h, cy - h)],
        1.0,
        Ink::Faint,
    ));
}

/// The dashed slot with its word, and the hollow station that ends the track of its row.
fn push_slot(nodes: &mut Vec<Node>, hot: bool, slot: Rect, plan: &Layout, lang: Lang) {
    let ink = if hot { Ink::Flow } else { Ink::Faint };
    nodes.push(outline(slot, ink, true));
    nodes.push(text(
        rect(slot.x, slot.y + SLOT_LABEL_DY, slot.w, 12.0),
        word(Text::ShelfSlot, lang),
        TextStyle::Label,
        ink,
        Align::Center,
    ));
    let track_y = plan
        .tracks
        .iter()
        .copied()
        .find(|&y| y > slot.y + slot.h)
        .unwrap_or(slot.y + slot.h);
    nodes.push(station(RIGHT, track_y, StationMark::Hollow, ink));
}

/// The footer: the count and the hint at the left, the undo word at the right.
fn push_footer(
    nodes: &mut Vec<Node>,
    targets: &mut Vec<HitTarget>,
    view: &ShelfView,
    plan: &Layout,
    lang: Lang,
) {
    let top = plan.footer_y - 10.0;
    nodes.push(text(
        rect(FOOTER_X, top, UNDO_X - FOOTER_X - 8.0, 12.0),
        &format!(
            "{} \u{b7} {}",
            files(view.items.len(), lang),
            word(Text::ShelfFooter, lang)
        ),
        TextStyle::Label,
        Ink::Faint,
        Align::Start,
    ));
    nodes.push(text(
        rect(CLEAR_X, top, CLEAR_W, 12.0),
        word(Text::ShelfClear, lang),
        TextStyle::Label,
        Ink::Muted,
        Align::End,
    ));
    targets.push(HitTarget {
        rect: rect(CLEAR_X, top - 2.0, CLEAR_W, 14.0),
        action: Action::ShelfClear,
    });
    // The undo word exists only while there is something to take back.
    if view.can_undo {
        nodes.push(text(
            rect(UNDO_X, top, UNDO_W, 12.0),
            word(Text::ShelfUndo, lang),
            TextStyle::Label,
            Ink::Muted,
            Align::End,
        ));
        targets.push(undo_target(UNDO_X, top - 2.0));
    }
}
