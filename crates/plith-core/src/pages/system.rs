//! The system page: monitor brightness on a ruler and the microphone as a route to its bus.
//!
//! Positions come from `systemPage()` and `micRow()` in `docs/design/r3/ray-paneli.html`. The
//! mock's text baselines become top-aligned rects (baseline minus roughly the cap height).

use super::kit::{line, rect, station, text};
use super::seek_fraction;
use super::tabbar::tabbar;
use super::{Action, HitTarget, PAGE_W, PageBuild, PageId};
use crate::draw::{Align, DisplayList, Ink, Node, Rect, StationMark, TextStyle};
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
const X0: f64 = 200.0;
/// Ruler and route line end x (mock `x1 = W - 70`).
const X1: f64 = PAGE_W - 70.0;
/// Brightness row y (mock `y = 80`).
const RULER_Y: f64 = 80.0;
/// Mic row y (mock `my = 132`).
const MIC_Y: f64 = 132.0;
/// Right edge of the value and bus texts (mock `W - 30`).
const RIGHT: f64 = PAGE_W - 30.0;
/// Reach of the ruler target above and below the line.
const RULER_GROW: f64 = 8.0;
/// Cut in the muted route: the gap runs from `X0 + 120` to `X0 + 170` (mock `micRow`).
const CUT_FROM: f64 = 120.0;
const CUT_TO: f64 = 170.0;

/// The ruler rectangle: the brightness hit area and the base of the percentage.
const RULER: Rect = Rect {
    x: X0,
    y: RULER_Y - 5.0,
    w: X1 - X0,
    h: 10.0,
};

/// The brightness percentage (0 to 100, rounded) for a click at `x` on `ruler`.
#[must_use]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn brightness_at(ruler: Rect, x: f64) -> u8 {
    (seek_fraction(ruler, x) * 100.0).round() as u8
}

/// The system page nodes, hit targets and island shape.
#[must_use]
pub fn system_page(facts: &SystemFacts, clock: &str) -> PageBuild {
    let (mut nodes, mut targets) = tabbar(PageId::System, clock);
    brightness_row(facts.brightness.as_ref(), &mut nodes, &mut targets);
    if let Some(mic) = &facts.mic {
        mic_row(mic, facts.mic_bus.as_deref(), &mut nodes, &mut targets);
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
    nodes: &mut Vec<Node>,
    targets: &mut Vec<HitTarget>,
) {
    let name_ink = if brightness.is_some() {
        Ink::Ink
    } else {
        Ink::Faint
    };
    nodes.push(text(
        rect(30.0, 70.0, 160.0, 16.0),
        "Parlakl\u{131}k",
        TextStyle::Name,
        name_ink,
        Align::Start,
    ));
    let Some((monitor, percent)) = brightness.map(|(m, p)| (m, (*p).min(100))) else {
        nodes.push(text(
            rect(X0, RULER_Y - 6.0, RIGHT - X0, 12.0),
            super::copy::DDC_OFF,
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
    let fraction = f64::from(percent) / 100.0;
    let at = X0 + (X1 - X0) * fraction;
    nodes.push(line(vec![(X0, RULER_Y), (X1, RULER_Y)], 1.0, Ink::Track));
    for i in 0..=10u8 {
        let x = X0 + (X1 - X0) * f64::from(i) / 10.0;
        let len = if i % 5 == 0 { 11.0 } else { 7.0 };
        nodes.push(line(
            vec![(x, RULER_Y + 4.0), (x, RULER_Y + len)],
            1.0,
            Ink::Faint,
        ));
    }
    nodes.push(line(vec![(X0, RULER_Y), (at, RULER_Y)], 2.0, Ink::Ink));
    nodes.push(station(at, RULER_Y, StationMark::Filled, Ink::Ink));
    nodes.push(text(
        rect(RIGHT - 60.0, RULER_Y - 8.0, 60.0, 14.0),
        &percent.to_string(),
        // The mock sets this value in mono 12, the clock's face and size.
        TextStyle::Clock,
        Ink::Ink,
        Align::End,
    ));
    targets.push(HitTarget {
        rect: rect(
            X0,
            RULER.y - RULER_GROW,
            RULER.w,
            RULER.h + 2.0 * RULER_GROW,
        ),
        action: Action::BrightnessRuler(RULER),
    });
}

fn mic_row(mic: &MicFacts, bus: Option<&str>, nodes: &mut Vec<Node>, targets: &mut Vec<HitTarget>) {
    nodes.push(text(
        rect(30.0, MIC_Y - 10.0, 160.0, 16.0),
        "Mikrofon",
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
            super::copy::OFF_LABEL,
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
