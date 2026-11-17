use super::*;

const KINDS: [DeviceKind; 3] = [
    DeviceKind::Headphones,
    DeviceKind::Speaker,
    DeviceKind::Display,
];

fn points(node: &Node) -> &[(f64, f64)] {
    match node {
        Node::Stroke { points, .. } | Node::Polygon { points, .. } => points,
        _ => &[],
    }
}

#[test]
fn every_point_stays_in_the_box() {
    for kind in KINDS {
        let nodes = glyph(kind, 0.0, 0.0, Ink::Ink);
        assert!(!nodes.is_empty(), "{kind:?}");
        for p in nodes.iter().flat_map(points) {
            assert!(
                (0.0..=GLYPH).contains(&p.0) && (0.0..=GLYPH).contains(&p.1),
                "{kind:?} {p:?}"
            );
        }
    }
}

#[test]
fn the_origin_moves_every_point() {
    for kind in KINDS {
        let at_zero = glyph(kind, 0.0, 0.0, Ink::Ink);
        let moved = glyph(kind, 30.0, 76.0, Ink::Ink);
        assert_eq!(at_zero.len(), moved.len());
        for (a, b) in at_zero.iter().zip(&moved) {
            for (pa, pb) in points(a).iter().zip(points(b)) {
                assert!((pa.0 + 30.0 - pb.0).abs() < 1e-9);
                assert!((pa.1 + 76.0 - pb.1).abs() < 1e-9);
            }
        }
    }
}

#[test]
fn the_ink_is_the_one_passed_in() {
    for kind in KINDS {
        for node in glyph(kind, 0.0, 0.0, Ink::Muted) {
            match node {
                Node::Stroke { ink, .. } | Node::Polygon { ink, .. } => {
                    assert_eq!(ink, Ink::Muted);
                }
                other => panic!("unexpected node {other:?}"),
            }
        }
    }
}

#[test]
fn the_three_glyphs_differ() {
    let a = glyph(DeviceKind::Headphones, 0.0, 0.0, Ink::Ink);
    let b = glyph(DeviceKind::Speaker, 0.0, 0.0, Ink::Ink);
    let c = glyph(DeviceKind::Display, 0.0, 0.0, Ink::Ink);
    assert_ne!(a, b);
    assert_ne!(b, c);
    assert_ne!(a, c);
}
