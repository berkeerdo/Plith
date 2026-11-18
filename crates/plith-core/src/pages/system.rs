//! The system page: monitor brightness on a ruler and the microphone as a route to its bus.
//!
//! Positions come from `systemPage()` and `micRow()` in `docs/design/r3/ray-paneli.html`. The
//! mock's text baselines become top-aligned rects (baseline minus roughly the cap height).

use super::kit::{line, rect, station, text};
use super::seek_fraction;
use super::tabbar::tabbar;
use super::{Action, HitTarget, PAGE_W, PageBuild, PageId};
use crate::draw::{Align, DisplayList, Ink, Node, Rect, StationMark, TextStyle};
use crate::lang::{Lang, Text, percent, text as word};
use crate::shapes;

/// What the system page shows.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SystemFacts {
    /// The monitor name and its brightness (0 to 100), or `None` when DDC/CI is unavailable.
    pub brightness: Option<(String, u8)>,
    /// The capture device, or `None` when there is none (the row is then absent).
    pub mic: Option<MicFacts>,
    /// The bus label the microphone feeds (for example `B1`).
    pub mic_bus: Option<String>,
}

/// The capture device row.
#[derive(Debug, Clone, PartialEq)]
pub struct MicFacts {
    /// The text under the name (hardware slot and device name).
    pub name: String,
    /// Whether the device is muted.
    pub muted: bool,
}

/// Page height (mock `systemPage`, `h = 178`).
const HEIGHT: f64 = 178.0;
/// Ruler and route line start x (mock `x0 = 200`).
pub(crate) const X0: f64 = 200.0;
/// Ruler and route line end x (mock `x1 = W - 70`).
pub(super) const X1: f64 = PAGE_W - 70.0;
/// Brightness row y (mock `y = 80`).
const RULER_Y: f64 = 80.0;
/// Mic row y (mock `my = 132`).
const MIC_Y: f64 = 132.0;
/// Right edge of the value and bus texts (mock `W - 30`).
pub(crate) const RIGHT: f64 = PAGE_W - 30.0;
/// Width of a row label and of the value boxes at the right margin.
pub(crate) const LABEL_W: f64 = 160.0;
pub(crate) const VALUE_W: f64 = 60.0;
/// Reach of the ruler target above and below the line.
const RULER_GROW: f64 = 8.0;
/// Cut in the muted route: the gap runs from `X0 + 120` to `X0 + 170` (mock `micRow`).
pub(crate) const CUT_FROM: f64 = 120.0;
pub(crate) const CUT_TO: f64 = 170.0;

/// The percentage (0 to 100, rounded) for a click at `x` on `ruler`; brightness and volume
/// share it.
#[must_use]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn brightness_at(ruler: Rect, x: f64) -> u8 {
    (seek_fraction(ruler, x) * 100.0).round() as u8
}

/// The system page nodes, hit targets and island shape.
#[must_use]
pub fn system_page(facts: &SystemFacts, clock: &str, lang: Lang) -> PageBuild {
    let (mut nodes, mut targets) = tabbar(Some(PageId::System), clock, lang);
    brightness_row(facts.brightness.as_ref(), lang, &mut nodes, &mut targets);
    if let Some(mic) = &facts.mic {
        mic_row(
            mic,
            facts.mic_bus.as_deref(),
            lang,
            &mut nodes,
            &mut targets,
        );
    }
    let mut shape = shapes::PAGES[PageId::System.index()];
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

fn brightness_row(
    brightness: Option<&(String, u8)>,
    lang: Lang,
    nodes: &mut Vec<Node>,
    targets: &mut Vec<HitTarget>,
) {
    let name_ink = if brightness.is_some() {
        Ink::Ink
    } else {
        Ink::Faint
    };
    nodes.push(text(
        rect(30.0, 70.0, LABEL_W, 16.0),
        word(Text::Brightness, lang),
        TextStyle::Name,
        name_ink,
        Align::Start,
    ));
    let Some((monitor, percent)) = brightness.map(|(m, p)| (m, (*p).min(100))) else {
        nodes.push(text(
            rect(X0, RULER_Y - 6.0, RIGHT - X0, 12.0),
            word(Text::DdcOff, lang),
            TextStyle::Label,
            Ink::Faint,
            Align::Start,
        ));
        return;
    };
    nodes.push(text(
        rect(30.0, 87.0, 160.0, 12.0),
        monitor,
        TextStyle::Sub,
        Ink::Faint,
        Align::Start,
    ));
    let span = (X0, X1);
    draw_ruler(nodes, span, RULER_Y, f64::from(percent) / 100.0, Ink::Ink);
    nodes.push(percent_value(RULER_Y, percent, lang, Ink::Ink));
    targets.push(ruler_target(
        span,
        RULER_Y,
        Action::BrightnessRuler(ruler_rect(span, RULER_Y)),
    ));
}

/// The ruler rectangle over `span` (start x, end x) at line `y`: the hit area and the base of
/// the percentage.
pub(super) fn ruler_rect(span: (f64, f64), y: f64) -> Rect {
    Rect {
        x: span.0,
        y: y - 5.0,
        w: span.1 - span.0,
        h: 10.0,
    }
}

/// The hit target of a ruler at line `y`, taller than the line by `RULER_GROW` each way.
pub(super) fn ruler_target(span: (f64, f64), y: f64, action: Action) -> HitTarget {
    let r = ruler_rect(span, y);
    HitTarget {
        rect: rect(span.0, r.y - RULER_GROW, r.w, r.h + 2.0 * RULER_GROW),
        action,
    }
}

/// The ruler: track, eleven ticks, the filled part up to `fraction` (0 to 1) and the square
/// station thumb, the fill and thumb in `ink`.
pub(super) fn draw_ruler(
    nodes: &mut Vec<Node>,
    (x0, x1): (f64, f64),
    y: f64,
    fraction: f64,
    ink: Ink,
) {
    let at = x0 + (x1 - x0) * fraction;
    nodes.push(line(vec![(x0, y), (x1, y)], 1.0, Ink::Track));
    for i in 0..=10u8 {
        let x = x0 + (x1 - x0) * f64::from(i) / 10.0;
        let len = if i % 5 == 0 { 11.0 } else { 7.0 };
        nodes.push(line(vec![(x, y + 4.0), (x, y + len)], 1.0, Ink::Faint));
    }
    nodes.push(line(vec![(x0, y), (at, y)], 2.0, ink));
    nodes.push(station(at, y, StationMark::Filled, ink));
}

/// The value at the right of a ruler: `%70` in Turkish, `70%` in English.
pub(super) fn percent_value(y: f64, value: u8, lang: Lang, ink: Ink) -> Node {
    text(
        rect(RIGHT - VALUE_W, y - 8.0, VALUE_W, 14.0),
        &percent(value, lang),
        // The mock sets this value in mono 12, the clock's face and size.
        TextStyle::Clock,
        ink,
        Align::End,
    )
}

fn mic_row(
    mic: &MicFacts,
    bus: Option<&str>,
    lang: Lang,
    nodes: &mut Vec<Node>,
    targets: &mut Vec<HitTarget>,
) {
    nodes.push(text(
        rect(30.0, MIC_Y - 10.0, LABEL_W, 16.0),
        word(Text::Microphone, lang),
        TextStyle::Name,
        Ink::Ink,
        Align::Start,
    ));
    nodes.push(text(
        rect(30.0, MIC_Y + 7.0, 160.0, 12.0),
        &mic.name,
        TextStyle::Sub,
        Ink::Faint,
        Align::Start,
    ));
    if mic.muted {
        nodes.push(station(X0, MIC_Y, StationMark::Filled, Ink::Ink));
        nodes.push(line(
            vec![(X0, MIC_Y), (X0 + CUT_FROM, MIC_Y)],
            1.0,
            Ink::Track,
        ));
        nodes.push(line(
            vec![(X0 + CUT_TO, MIC_Y), (X1, MIC_Y)],
            1.0,
            Ink::Track,
        ));
        let mid = X0 + f64::midpoint(CUT_FROM, CUT_TO);
        nodes.push(text(
            rect(mid - 30.0, MIC_Y - 6.0, 60.0, 12.0),
            word(Text::Off, lang),
            TextStyle::Label,
            Ink::Ink,
            Align::Center,
        ));
        nodes.push(station(X1, MIC_Y, StationMark::Hollow, Ink::Faint));
    } else {
        nodes.push(station(X0, MIC_Y, StationMark::Filled, Ink::Flow));
        nodes.push(line(vec![(X0, MIC_Y), (X1, MIC_Y)], 2.0, Ink::Flow));
        nodes.push(station(X1, MIC_Y, StationMark::Filled, Ink::Flow));
    }
    if let Some(bus) = bus {
        nodes.push(text(
            rect(RIGHT - 60.0, MIC_Y - 6.0, 60.0, 12.0),
            bus,
            TextStyle::Label,
            Ink::Muted,
            Align::End,
        ));
    }
    targets.push(HitTarget {
        rect: rect(30.0, MIC_Y - 14.0, RIGHT - 30.0, 36.0),
        action: Action::ToggleMic,
    });
}

#[cfg(test)]
mod tests;
