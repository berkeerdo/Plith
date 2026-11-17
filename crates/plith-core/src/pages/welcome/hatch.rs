//! The lithology marks of the weather band: cloud dashes, rain dots, snow crosses.

use crate::draw::{Ink, Node};
use crate::pages::kit::{line, rect};
use crate::weather::Sky;

/// The lithology marks of a sky inside the box `(left, top, right, bottom)`.
pub(super) fn hatch(
    out: &mut Vec<Node>,
    sky: Sky,
    (l, t, r, b): (f64, f64, f64, f64),
    logged: bool,
) {
    let ink = if logged { Ink::Muted } else { Ink::Faint };
    let (cw, ch) = match sky {
        Sky::Clear => return,
        Sky::Cloud => (6.0, 4.0),
        Sky::Rain => (6.0, 5.0),
        Sky::Snow => (8.0, 8.0),
    };
    let fits = |x: f64, y: f64, w: f64, h: f64| x >= l && y >= t && x + w <= r && y + h <= b;
    let dot = |x: f64, y: f64, w: f64| Node::Fill {
        rect: rect(x, y, w, 1.0),
        radius: 0.0,
        ink,
    };
    let mut gy = (t / ch).floor() * ch;
    while gy < b {
        let mut gx = (l / cw).floor() * cw;
        while gx < r {
            match sky {
                Sky::Cloud if fits(gx, gy + 1.0, 4.0, 1.0) => out.push(dot(gx, gy + 1.0, 4.0)),
                Sky::Rain => {
                    for (dx, dy) in [(1.0, 1.0), (4.0, 3.0)] {
                        if fits(gx + dx, gy + dy, 1.0, 1.0) {
                            out.push(dot(gx + dx, gy + dy, 1.0));
                        }
                    }
                }
                Sky::Snow if fits(gx + 2.0, gy + 2.0, 3.0, 3.0) => {
                    for pts in [
                        vec![(gx + 2.0, gy + 2.0), (gx + 5.0, gy + 5.0)],
                        vec![(gx + 5.0, gy + 2.0), (gx + 2.0, gy + 5.0)],
                    ] {
                        out.push(line(pts, 1.0, ink));
                    }
                }
                _ => {}
            }
            gx += cw;
        }
        gy += ch;
    }
}
