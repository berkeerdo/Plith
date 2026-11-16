//! Small node constructors shared by the system, shelf and pill layouts.

use std::sync::Arc;

use crate::draw::{Align, Ink, Node, Rect, StationMark, TextStyle, Trim};

/// A rectangle from its parts.
pub(super) fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
    Rect { x, y, w, h }
}

/// A text node, cut at the end when too long.
pub(super) fn text(r: Rect, s: &str, style: TextStyle, ink: Ink, align: Align) -> Node {
    Node::Text {
        rect: r,
        text: Arc::from(s),
        style,
        ink,
        align,
        trim: Trim::End,
    }
}

/// A solid polyline.
pub(super) fn line(points: Vec<(f64, f64)>, width: f64, ink: Ink) -> Node {
    Node::Stroke {
        points,
        width,
        ink,
        dashed: false,
    }
}

/// A station marker.
pub(super) fn station(x: f64, y: f64, mark: StationMark, ink: Ink) -> Node {
    Node::Station { x, y, mark, ink }
}
