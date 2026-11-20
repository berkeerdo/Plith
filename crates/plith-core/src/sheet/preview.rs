//! A scaled resting pill preview: the pill's raised floor with a 1 DIP border, and its list
//! inset on top. Shared by the sheet's Idle section and any later place that shows previews.

use crate::draw::{DisplayList, Ink, Node, Rect};
use crate::pages::idle_pill::rest_shape;
use crate::settings::IdleLook;
use crate::shapes::REST;
use std::sync::Arc;

/// The nodes of the `look` preview with its top left at (`x`, `y`), drawn at `scale`. The pill
/// is a border-ink rounded rectangle with a raised floor 1 DIP inside it, so it stands out from
/// the floor around it; Hidden is a faint line instead. `list` is inset over it.
pub(crate) fn pill_preview(
    look: IdleLook,
    list: Arc<DisplayList>,
    (x, y): (f64, f64),
    scale: f64,
) -> Vec<Node> {
    let shape = rest_shape(look);
    let (w, h) = (shape.width * scale, shape.height * scale);
    let radius = shape.drawn_radius() * scale;
    let mut nodes = if look == IdleLook::Hidden {
        vec![Node::Stroke {
            points: vec![(x, y + 1.0), (x + REST.width * scale, y + 1.0)],
            width: 1.0,
            ink: Ink::Faint,
            dashed: false,
        }]
    } else {
        vec![
            Node::Fill {
                rect: Rect { x, y, w, h },
                radius,
                ink: Ink::Border,
            },
            Node::Fill {
                rect: Rect {
                    x: x + 1.0,
                    y: y + 1.0,
                    w: w - 2.0,
                    h: h - 2.0,
                },
                radius: (radius - 1.0).max(0.0),
                ink: Ink::FloorHigh,
            },
        ]
    };
    nodes.push(Node::Inset { list, x, y, scale });
    nodes
}
