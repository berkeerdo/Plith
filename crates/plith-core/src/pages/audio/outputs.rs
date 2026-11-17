//! The audio page without Voicemeeter (ADR 0013): the default output on top with a volume ruler
//! in the grammar of the system page's brightness row, then the other outputs, each a click away
//! from becoming the default.

use super::{AudioView, OutputRow, OutputValue};
use crate::draw::{Align, DisplayList, Ink, Node, TextStyle, Trim};
use crate::pages::copy;
use crate::pages::glyphs::{GLYPH, glyph};
use crate::pages::kit::{rect, text, text_trim};
use crate::pages::system::{RIGHT, draw_ruler, percent_value, ruler_rect, ruler_target};
use crate::pages::tabbar::tabbar;
use crate::pages::{Action, HitTarget, MARGIN_LEFT, PAGE_W, PageBuild, PageId};
use crate::shapes;

/// Ruler line y, the same as the brightness row (mock `systemPage`, `y = 80`).
const DEFAULT_Y: f64 = 80.0;
/// Centre y of the `DİĞER ÇIKIŞLAR` label.
const LABEL_Y: f64 = 118.0;
/// Centre y of the first other output.
const FIRST_OTHER_Y: f64 = 146.0;
/// Distance between other outputs.
const OTHER_PITCH: f64 = 40.0;
/// Page height below the last row centre.
const BOTTOM: f64 = 60.0;
/// Ruler start x: far enough right for a `VARSAYILAN · KULAKLIK` caption to clear it.
const RULER_X0: f64 = 264.0;
/// Ruler end x: leaves room for `%62` and `SESSİZ` at the right margin.
const RULER_X1: f64 = PAGE_W - 90.0;
/// The ruler span.
const SPAN: (f64, f64) = (RULER_X0, RULER_X1);
/// Page height with no output at all.
const HEIGHT_EMPTY: f64 = 112.0;
/// Page height with the default output only.
const HEIGHT_DEFAULT_ONLY: f64 = 146.0;
/// Footer text top above the page bottom.
const FOOTER_UP: f64 = 32.0;
/// Left edge of a name and its caption, after the glyph.
const NAME_X: f64 = MARGIN_LEFT + GLYPH + 6.0;
/// Width of a name or caption: up to 8 DIP before the ruler start.
const NAME_W: f64 = RULER_X0 - 8.0 - NAME_X;
/// Text heights.
const NAME_H: f64 = 18.0;
const SUB_H: f64 = 12.0;
/// Half height of an other output's hit target.
const ROW_HIT_HALF: f64 = 18.0;
/// Centre y of the default row's name and caption pair (name top 70 to caption bottom 99).
const PAIR_CENTRE: f64 = 84.5;

/// Other outputs that fit the maximum page height.
fn max_others() -> usize {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    // The quotient is a small positive number.
    let rows = ((shapes::PAGE_MAX_HEIGHT - BOTTOM - FIRST_OTHER_Y) / OTHER_PITCH).floor() as usize;
    rows + 1
}

/// Centre y of other output `i`.
fn other_y(i: usize) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    // Row counts are tiny, the cast is exact.
    let i = i as f64;
    FIRST_OTHER_Y + i * OTHER_PITCH
}

/// The outputs page nodes, hit targets and island shape.
pub(super) fn page(view: &AudioView, clock: &str) -> PageBuild {
    let (mut nodes, mut targets) = tabbar(Some(PageId::Audio), clock);
    let default = view.outputs.iter().find(|o| o.default);
    match default {
        Some(row) => default_row(row, &mut nodes, &mut targets),
        None => nodes.push(text(
            rect(MARGIN_LEFT, 70.0, RIGHT - MARGIN_LEFT, 16.0),
            copy::NO_OUTPUT,
            TextStyle::Name,
            Ink::Faint,
            Align::Start,
        )),
    }
    let others: Vec<&OutputRow> = view.outputs.iter().filter(|o| !o.default).collect();
    let height = if view.outputs.is_empty() {
        // Nothing to click: no footer, the page is just the empty-state line.
        HEIGHT_EMPTY
    } else {
        let height = others_block(&others, &mut nodes, &mut targets);
        nodes.push(text_trim(
            rect(
                MARGIN_LEFT,
                height - FOOTER_UP,
                PAGE_W - 2.0 * MARGIN_LEFT,
                SUB_H,
            ),
            view.footer.text(),
            TextStyle::Label,
            Ink::Faint,
            Align::Start,
            Trim::End,
        ));
        height
    };
    let mut shape = shapes::PAGES[PageId::Audio.index()];
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

/// Glyph, name, caption, ruler and value of the default output.
fn default_row(row: &OutputRow, nodes: &mut Vec<Node>, targets: &mut Vec<HitTarget>) {
    nodes.extend(glyph(
        row.kind,
        MARGIN_LEFT,
        PAIR_CENTRE - GLYPH / 2.0,
        Ink::Ink,
    ));
    nodes.push(text_trim(
        rect(NAME_X, 70.0, NAME_W, NAME_H),
        &row.name,
        TextStyle::Name,
        Ink::Ink,
        Align::Start,
        Trim::End,
    ));
    nodes.push(text_trim(
        rect(NAME_X, 87.0, NAME_W, SUB_H),
        &format!("{} \u{b7} {}", copy::DEFAULT_CAPTION, row.kind.caption()),
        TextStyle::Sub,
        Ink::Faint,
        Align::Start,
        Trim::End,
    ));
    let (percent, ink) = match (row.muted, row.value) {
        (false, OutputValue::Percent(p)) => (Some(p.min(100)), Ink::Flow),
        (_, OutputValue::Percent(p)) => (Some(p.min(100)), Ink::Faint),
        _ => (None, Ink::Faint),
    };
    let Some(percent) = percent else {
        return;
    };
    draw_ruler(nodes, SPAN, DEFAULT_Y, f64::from(percent) / 100.0, ink);
    if row.muted {
        nodes.push(text(
            rect(RIGHT - 60.0, DEFAULT_Y - 6.0, 60.0, SUB_H),
            copy::MUTED_LABEL,
            TextStyle::Label,
            Ink::Faint,
            Align::End,
        ));
    } else {
        nodes.push(percent_value(DEFAULT_Y, percent, Ink::Ink));
    }
    targets.push(ruler_target(
        SPAN,
        DEFAULT_Y,
        Action::VolumeRuler(ruler_rect(SPAN, DEFAULT_Y)),
    ));
}

/// The label and the rows of the other outputs, with a `+N` line for rows past the page;
/// returns the page height.
fn others_block(others: &[&OutputRow], nodes: &mut Vec<Node>, targets: &mut Vec<HitTarget>) -> f64 {
    if others.is_empty() {
        return HEIGHT_DEFAULT_ONLY;
    }
    nodes.push(text(
        rect(MARGIN_LEFT, LABEL_Y - 6.0, RIGHT - MARGIN_LEFT, SUB_H),
        copy::OTHER_OUTPUTS,
        TextStyle::Label,
        Ink::Faint,
        Align::Start,
    ));
    let cap = max_others();
    let shown = if others.len() > cap {
        cap - 1
    } else {
        others.len()
    };
    for (i, row) in others.iter().take(shown).enumerate() {
        other_row(i, row, nodes, targets);
    }
    let mut rows = shown;
    if others.len() > shown {
        nodes.push(text(
            rect(NAME_X, other_y(shown) - 6.0, NAME_W, SUB_H),
            &format!("+{}", others.len() - shown),
            TextStyle::Label,
            Ink::Faint,
            Align::Start,
        ));
        rows += 1;
    }
    (other_y(rows - 1) + BOTTOM).min(shapes::PAGE_MAX_HEIGHT)
}

/// One other output: glyph, name and kind caption; the whole row makes it the default.
fn other_row(i: usize, row: &OutputRow, nodes: &mut Vec<Node>, targets: &mut Vec<HitTarget>) {
    let y = other_y(i);
    nodes.extend(glyph(row.kind, MARGIN_LEFT, y - GLYPH / 2.0, Ink::Muted));
    nodes.push(text_trim(
        rect(NAME_X, y - 14.0, NAME_W, NAME_H),
        &row.name,
        TextStyle::Body,
        Ink::Muted,
        Align::Start,
        Trim::Middle,
    ));
    nodes.push(text(
        rect(NAME_X, y + 4.0, NAME_W, SUB_H),
        row.kind.caption(),
        TextStyle::Sub,
        Ink::Faint,
        Align::Start,
    ));
    if let Some(id) = &row.id {
        targets.push(HitTarget {
            rect: rect(
                MARGIN_LEFT,
                y - ROW_HIT_HALF,
                RIGHT - MARGIN_LEFT,
                2.0 * ROW_HIT_HALF,
            ),
            action: Action::SetDefaultEndpoint(id.clone()),
        });
    }
}

#[cfg(test)]
mod tests;
