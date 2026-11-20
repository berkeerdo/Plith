//! Line glyphs for output devices, drawn from display-list strokes inside a 16 by 16 DIP box.
//! Pure functions: the caller picks the origin and the ink, and every point stays in the box.

use super::audio::DeviceKind;
use super::kit::line;
use crate::draw::{Ink, Node};

/// Side of a glyph box in DIP.
pub const GLYPH: f64 = 16.0;
/// Stroke width of every glyph.
const WIDTH: f64 = 1.5;
/// Segments of a drawn arc.
const ARC_STEPS: u32 = 10;

/// The glyph of `kind` with its box origin at (`x`, `y`).
#[must_use]
pub fn glyph(kind: DeviceKind, x: f64, y: f64, ink: Ink) -> Vec<Node> {
    let mut nodes = match kind {
        DeviceKind::Headphones => headphones(ink),
        DeviceKind::Speaker => speaker(ink),
        DeviceKind::Display => display(ink),
    };
    for node in &mut nodes {
        match node {
            Node::Stroke { points, .. } | Node::Polygon { points, .. } => {
                for p in points {
                    *p = (p.0 + x, p.1 + y);
                }
            }
            _ => {}
        }
    }
    nodes
}

/// The home mark: a roof over a walled house with a door, box origin at (`x`, `y`).
#[must_use]
pub(crate) fn home(x: f64, y: f64, ink: Ink) -> Vec<Node> {
    let strokes = [
        vec![(1.5, 8.0), (8.0, 2.0), (14.5, 8.0)],
        vec![(3.5, 7.0), (3.5, 14.0), (12.5, 14.0), (12.5, 7.0)],
        vec![(6.5, 14.0), (6.5, 10.0), (9.5, 10.0), (9.5, 14.0)],
    ];
    strokes
        .into_iter()
        .map(|pts| {
            line(
                pts.into_iter().map(|p| (p.0 + x, p.1 + y)).collect(),
                WIDTH,
                ink,
            )
        })
        .collect()
}

/// Teeth of the gear mark.
const GEAR_TEETH: u32 = 8;

/// The gear mark: a toothed wheel around a hub circle, box origin at (`x`, `y`).
#[must_use]
pub(crate) fn gear(x: f64, y: f64, ink: Ink) -> Vec<Node> {
    let (cx, cy) = (x + GLYPH / 2.0, y + GLYPH / 2.0);
    let (outer, inner) = (7.25, 5.5);
    let step = 360.0 / f64::from(GEAR_TEETH);
    let mut wheel = Vec::new();
    for tooth in 0..GEAR_TEETH {
        let a = step * f64::from(tooth);
        for (r, da) in [(inner, -14.0), (outer, -8.0), (outer, 8.0), (inner, 14.0)] {
            let rad = (a + da).to_radians();
            wheel.push((cx + r * rad.cos(), cy - r * rad.sin()));
        }
    }
    wheel.push(wheel[0]);
    let hub = arc(cx, cy, 2.25, 0.0, 360.0);
    vec![line(wheel, WIDTH, ink), line(hub, WIDTH, ink)]
}

/// Points of an arc around (`cx`, `cy`) from `from` to `to` degrees (0 is right, 90 is up).
fn arc(cx: f64, cy: f64, r: f64, from: f64, to: f64) -> Vec<(f64, f64)> {
    (0..=ARC_STEPS)
        .map(|i| {
            let t = from + (to - from) * f64::from(i) / f64::from(ARC_STEPS);
            let rad = t.to_radians();
            (cx + r * rad.cos(), cy - r * rad.sin())
        })
        .collect()
}

/// A headband arc over two ear cups.
fn headphones(ink: Ink) -> Vec<Node> {
    vec![
        line(arc(8.0, 8.5, 6.0, 180.0, 0.0), WIDTH, ink),
        Node::Polygon {
            points: vec![(1.0, 8.5), (4.0, 8.5), (4.0, 14.0), (1.0, 14.0)],
            ink,
        },
        Node::Polygon {
            points: vec![(12.0, 8.5), (15.0, 8.5), (15.0, 14.0), (12.0, 14.0)],
            ink,
        },
    ]
}

/// A cabinet, a cone and two sound waves.
fn speaker(ink: Ink) -> Vec<Node> {
    vec![
        Node::Polygon {
            points: vec![(1.0, 6.0), (4.5, 6.0), (4.5, 10.0), (1.0, 10.0)],
            ink,
        },
        Node::Polygon {
            points: vec![(4.5, 6.0), (8.5, 2.5), (8.5, 13.5), (4.5, 10.0)],
            ink,
        },
        line(arc(8.5, 8.0, 3.5, 45.0, -45.0), WIDTH, ink),
        line(arc(8.5, 8.0, 6.5, 45.0, -45.0), WIDTH, ink),
    ]
}

/// A monitor outline on a stand.
fn display(ink: Ink) -> Vec<Node> {
    vec![
        line(
            vec![
                (1.0, 2.0),
                (15.0, 2.0),
                (15.0, 11.0),
                (1.0, 11.0),
                (1.0, 2.0),
            ],
            WIDTH,
            ink,
        ),
        line(vec![(8.0, 11.0), (8.0, 14.0)], WIDTH, ink),
        line(vec![(4.5, 14.5), (11.5, 14.5)], WIDTH, ink),
    ]
}

#[cfg(test)]
mod tests;
