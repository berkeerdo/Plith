//! The audio page layout: the view model drawn as a track diagram.
//!
//! Positions come from `audioPage()` and `audioNoVoicemeeter()` in
//! `docs/design/r3/ray-paneli.html`. Mock text baselines become top-aligned rects (baseline minus
//! the font size); stations sit 3 DIP above a row's baseline, as in the mock.

mod routes;

use super::{
    AudioView, DENSE_ABOVE, Footer, MAX_OUTPUT_SLOTS, OutputRow, OutputValue, PITCH_DENSE,
    PITCH_MIN, PITCH_NORMAL, PickMode, RouteState, SourceRow,
};
use crate::draw::{Align, DisplayList, Ink, LiveNode, LiveRail, Node, TextStyle, Trim};
use crate::pages::kit::{rect, text_trim};
use crate::pages::tabbar::tabbar;
use crate::pages::{Action, HitTarget, MARGIN_LEFT, PAGE_W, PageBuild, PageId};
use crate::shapes;

pub use routes::route_path;

/// Source station x (mock `XS = 252`).
const SOURCE_X: f64 = 252.0;
/// Output station x (mock `XO = 452`).
const OUTPUT_X: f64 = 452.0;
/// Right edge of the output values (mock `W - 24`).
const VALUE_RIGHT: f64 = PAGE_W - 24.0;
/// End of a cut stub and its hollow station (mock: 290).
const CUT_END_X: f64 = 290.0;
/// Left edge of the `SESSİZ` label on a cut line (mock: 300).
const CUT_LABEL_X: f64 = 300.0;
/// Baseline of the first row (mock `SOURCES[0].y = 86`).
const FIRST_ROW_Y: f64 = 86.0;
/// Station centre above a row baseline (mock `s.y - 3`).
const STATION_DY: f64 = 3.0;
/// Page height below the last row baseline (mock: 300 - 236).
const BOTTOM: f64 = 64.0;
/// Where a diagonal ends before the output station while the run fits (mock line 163: the
/// Discord to A1 rise runs from x 280 to 430 with `XO = 452`).
const DIAGONAL_END: f64 = 22.0;
/// Shortest horizontal leg at either end of a route.
const MIN_LEG: f64 = 6.0;
/// The tallest rise a route can draw at 45 degrees: 188.
const REACH: f64 = (OUTPUT_X - MIN_LEG) - (SOURCE_X + MIN_LEG);
/// Footer baseline above the page bottom (mock: 284 on a 300 high page).
const FOOTER_UP: f64 = 16.0;
/// Trace slot left edge and width (mock `trace(svg, 168, ...)`, `w = 64`).
const TRACE_X: f64 = 168.0;
const TRACE_W: f64 = 64.0;
/// Trace slot half height around the station line (mock clip notch `y - 8`, height 16).
const TRACE_HALF: f64 = 8.0;
/// Width of a source name, up to the trace slot.
const SOURCE_NAME_W: f64 = TRACE_X - 8.0 - MARGIN_LEFT;
/// Output tag x (mock `XO + 14`) and name x after a tag: the mock's `XO + 38` moved in to
/// `XO + 34` (the two-character mono tag ends near `XO + 29`), so device names keep 4 DIP more.
const TAG_X: f64 = OUTPUT_X + 14.0;
const TAG_NAME_X: f64 = OUTPUT_X + 34.0;
/// Advance of one value character (Martian Mono 10 with 0.06 em tracking), used to keep the
/// name clear of the value.
const VALUE_CHAR_W: f64 = 7.4;
/// Extra room on the value's left so DirectWrite never trims it; the text is end-aligned.
const VALUE_SLACK: f64 = 8.0;
/// Gap between an output name and its value.
const VALUE_GAP: f64 = 8.0;
/// Half height of a row hit target.
const ROW_HIT_HALF: f64 = 16.0;
/// Reach of row targets past a station.
const STATION_REACH: f64 = 6.0;
/// Text heights: Name, labels and values.
const NAME_H: f64 = 18.0;
const LABEL_H: f64 = 12.0;
const LABEL_TEXT: &str = crate::pages::copy::MUTED_LABEL;

/// Baseline of row `i`.
fn row_y(view: &AudioView, i: usize) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    // Row counts are tiny, the cast is exact.
    let i = i as f64;
    FIRST_ROW_Y + i * view.row_pitch
}

/// Station centre y of row `i`.
fn station_y(view: &AudioView, i: usize) -> f64 {
    row_y(view, i) - STATION_DY
}

/// The value copy: a gain as a bare signed whole number (`-6`, `0`, `+3`, like the mock's
/// `62`), a bare percent, or nothing.
fn format_value(value: OutputValue) -> String {
    match value {
        OutputValue::Db(db) => {
            #[allow(clippy::cast_possible_truncation)]
            // Gains are within -60..12 dB, the rounded value fits an i32.
            let whole = db.round() as i32;
            if whole > 0 {
                format!("+{whole}")
            } else {
                whole.to_string()
            }
        }
        OutputValue::Percent(p) => p.to_string(),
        OutputValue::None => String::new(),
    }
}

/// Row pitch for a diagram of `rows` rows (sources or outputs, whichever is more): 50 as the
/// mock; up to five rows no taller than the 45 degree reach allows (floor 40); 40 above five
/// rows; then shrunk (floor 32) until the page fits `PAGE_MAX_HEIGHT`.
#[must_use]
pub fn row_pitch(rows: usize) -> f64 {
    if rows <= 1 {
        return PITCH_NORMAL;
    }
    #[allow(clippy::cast_precision_loss)]
    // Row counts are tiny, the cast is exact.
    let gaps = (rows - 1) as f64;
    let pitch = if rows <= DENSE_ABOVE {
        (REACH / gaps).clamp(PITCH_DENSE, PITCH_NORMAL)
    } else {
        PITCH_DENSE
    };
    let fit = (shapes::PAGE_MAX_HEIGHT - FIRST_ROW_Y - BOTTOM) / gaps;
    pitch.min(fit).max(PITCH_MIN)
}

/// The slot each output row takes (`None` when hidden) and how many rows are hidden. Up to
/// `MAX_OUTPUT_SLOTS` rows all show; past that the first nine show and the last slot holds
/// `+N`. A routed output (the default endpoint) never hides: it takes the ninth slot.
fn output_slots(view: &AudioView) -> (Vec<Option<usize>>, usize) {
    let n = view.outputs.len();
    if n <= MAX_OUTPUT_SLOTS {
        return ((0..n).map(Some).collect(), 0);
    }
    let shown = MAX_OUTPUT_SLOTS - 1;
    let mut slots: Vec<Option<usize>> = (0..n).map(|o| (o < shown).then_some(o)).collect();
    let routed = |o: &usize| view.outputs[*o].used || view.outputs[*o].live;
    if !(0..shown).any(|o| routed(&o))
        && let Some(keep) = (shown..n).find(routed)
    {
        slots[shown - 1] = None;
        slots[keep] = Some(shown - 1);
    }
    (slots, n - shown)
}

/// The audio page nodes, hit targets and island shape.
#[must_use]
pub fn audio_page(view: &AudioView, clock: &str) -> PageBuild {
    if view.footer == Footer::Outputs {
        return super::outputs::page(view, clock);
    }
    let (mut nodes, mut targets) = tabbar(PageId::Audio, clock);
    let (slots, hidden) = output_slots(view);
    let output_rows = view.outputs.len().min(MAX_OUTPUT_SLOTS);
    let rows = view.sources.len().max(output_rows).max(1);
    let height = (row_y(view, rows - 1) + BOTTOM).min(shapes::PAGE_MAX_HEIGHT);
    for (i, row) in view.sources.iter().enumerate() {
        source_row(view, i, row, &mut nodes, &mut targets);
    }
    for (o, row) in view.outputs.iter().enumerate() {
        if let Some(slot) = slots[o] {
            output_row(view, (o, slot), row, &mut nodes, &mut targets);
        }
    }
    if hidden > 0 {
        nodes.push(text_trim(
            rect(
                TAG_X,
                row_y(view, output_rows - 1) - 9.0,
                VALUE_RIGHT - TAG_X,
                LABEL_H,
            ),
            &format!("+{hidden}"),
            TextStyle::Label,
            Ink::Faint,
            Align::Start,
            Trim::End,
        ));
    }
    routes::draw(view, &slots, &mut nodes);
    if let PickMode::Picked(_) = view.mode {
        let top = row_y(view, 0) - ROW_HIT_HALF;
        let left = SOURCE_X + 2.0 * STATION_REACH;
        targets.push(HitTarget {
            rect: rect(
                left,
                top,
                OUTPUT_X - 2.0 * STATION_REACH - left,
                row_y(view, rows - 1) + ROW_HIT_HALF - top,
            ),
            action: Action::CancelPick,
        });
    }
    let footer_y = height - FOOTER_UP - 10.0;
    nodes.push(text_trim(
        rect(MARGIN_LEFT, footer_y, PAGE_W - 2.0 * MARGIN_LEFT, LABEL_H),
        view.footer.text(),
        TextStyle::Label,
        Ink::Faint,
        Align::Start,
        Trim::End,
    ));
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

/// Name, caption and trace slot on the left; the station is drawn with the routes.
fn source_row(
    view: &AudioView,
    i: usize,
    row: &SourceRow,
    nodes: &mut Vec<Node>,
    targets: &mut Vec<HitTarget>,
) {
    let y = row_y(view, i);
    let (name, ink) = if row.picked {
        (format!("[ {} ]", row.name), Ink::Flow)
    } else if row.muted {
        (row.name.clone(), Ink::Faint)
    } else {
        (row.name.clone(), Ink::Ink)
    };
    nodes.push(text_trim(
        rect(MARGIN_LEFT, y - 12.0, SOURCE_NAME_W, NAME_H),
        &name,
        TextStyle::Name,
        ink,
        Align::Start,
        Trim::End,
    ));
    nodes.push(text_trim(
        rect(MARGIN_LEFT, y + 5.0, SOURCE_X - 8.0 - MARGIN_LEFT, LABEL_H),
        &row.sub,
        TextStyle::Sub,
        Ink::Faint,
        Align::Start,
        Trim::End,
    ));
    if !row.muted {
        let sy = station_y(view, i);
        nodes.push(Node::Live(LiveNode::Trace {
            rect: rect(TRACE_X, sy - TRACE_HALF, TRACE_W, 2.0 * TRACE_HALF),
            rail: LiveRail::Strip,
            index: i,
        }));
    }
    targets.push(HitTarget {
        rect: rect(
            MARGIN_LEFT,
            y - ROW_HIT_HALF,
            SOURCE_X + STATION_REACH - MARGIN_LEFT,
            2.0 * ROW_HIT_HALF,
        ),
        action: Action::PickSource(i),
    });
}

/// Tag, name and value of output `o` in row `slot`; the station is drawn with the routes.
fn output_row(
    view: &AudioView,
    (o, slot): (usize, usize),
    row: &OutputRow,
    nodes: &mut Vec<Node>,
    targets: &mut Vec<HitTarget>,
) {
    let y = row_y(view, slot);
    let (value, style, ink) = if row.muted {
        (LABEL_TEXT.to_string(), TextStyle::Label, Ink::Faint)
    } else {
        (format_value(row.value), TextStyle::Value, Ink::Muted)
    };
    let mut name_right = VALUE_RIGHT;
    if !value.is_empty() {
        #[allow(clippy::cast_precision_loss)]
        // A value is a few characters, the cast is exact.
        let w = value.chars().count() as f64 * VALUE_CHAR_W;
        name_right = VALUE_RIGHT - w - VALUE_GAP;
        nodes.push(text_trim(
            rect(
                VALUE_RIGHT - w - VALUE_SLACK,
                y - 9.0,
                w + VALUE_SLACK,
                LABEL_H,
            ),
            &value,
            style,
            ink,
            Align::End,
            Trim::End,
        ));
    }
    // The mock: a named bus reads in ink even with no route; only the unused bus is faint.
    let name_ink = if row.unnamed { Ink::Faint } else { Ink::Ink };
    let name_x = match &row.tag {
        Some(tag) => {
            nodes.push(text_trim(
                rect(TAG_X, y - 9.0, TAG_NAME_X - TAG_X, LABEL_H),
                tag,
                TextStyle::Label,
                Ink::Faint,
                Align::Start,
                Trim::End,
            ));
            (row.name != *tag).then_some(TAG_NAME_X)
        }
        None => Some(TAG_X),
    };
    if let Some(x) = name_x {
        nodes.push(text_trim(
            rect(x, y - 10.0, (name_right - x).max(0.0), NAME_H),
            &row.name,
            TextStyle::Body,
            name_ink,
            Align::Start,
            Trim::Middle,
        ));
    }
    let left = OUTPUT_X - STATION_REACH;
    targets.push(HitTarget {
        rect: rect(
            left,
            y - ROW_HIT_HALF,
            VALUE_RIGHT - left,
            2.0 * ROW_HIT_HALF,
        ),
        action: Action::PickOutput(o),
    });
}

/// Whether any route of `state` starts at source `s`.
fn source_has(view: &AudioView, s: usize, state: RouteState) -> bool {
    view.routes
        .iter()
        .any(|r| r.source == s && r.state == state)
}

#[cfg(test)]
mod tests;
