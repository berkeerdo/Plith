#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
//! Offscreen pixel tests for the marks the R3 mock adds: filled polygons, the seek head and the
//! trace baseline. WARP, no window.

use plith_core::ambient::{LiveValues, TraceSamples};
use plith_core::draw::{
    DisplayList, Ink, LiveNode, LiveRail, Node, PageContent, Rect, SeekTimeline,
};
use plith_core::geometry::IslandShape;
use plith_core::page::{PageLayer, PageLayers};
use plith_core::theme::{Palette, Theme};
use plith_render::paint::PaintFrame;
use plith_render::{TextSystem, render_offscreen};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

const SHAPE: IslandShape = IslandShape {
    width: 400.0,
    height: 160.0,
    radius: 24.0,
};
const W: u32 = 400;
const H: u32 = 160;

fn page_frame(nodes: Vec<Node>, live: LiveValues) -> PaintFrame {
    let mut content = PageContent::empty();
    content.pages[0] = Arc::new(DisplayList {
        nodes,
        size: (SHAPE.width, SHAPE.height),
    });
    PaintFrame {
        shape: SHAPE,
        origin: (0.0, 0.0),
        pages: PageLayers::of(&[PageLayer {
            page: 0,
            offset: 0.0,
            opacity: 1.0,
        }]),
        content_opacity: 1.0,
        content: Arc::new(content),
        content_gen: 0,
        live,
    }
}

fn render(f: &PaintFrame) -> Vec<u8> {
    let text = TextSystem::new().expect("text system");
    render_offscreen(W, H, 1.0, f, Palette::default(), text).unwrap()
}

fn pixel(buf: &[u8], x: u32, y: u32) -> [u8; 4] {
    let i = ((y * W + x) * 4) as usize;
    [buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]
}

fn bgra(c: [f32; 4]) -> [u8; 4] {
    let b = |v: f32| (v * 255.0).round() as u8;
    [b(c[2]), b(c[1]), b(c[0]), 255]
}

fn near(got: [u8; 4], want: [u8; 4], tol: u8, what: &str) {
    let ok = got.iter().zip(want).all(|(g, w)| g.abs_diff(w) <= tol);
    assert!(ok, "{what}: got {got:?}, want {want:?} within {tol}");
}

#[test]
fn polygon_fills_its_triangle() {
    let p = Theme::Gece.palette();
    let triangle = Node::Polygon {
        points: vec![(100.0, 40.0), (140.0, 60.0), (100.0, 80.0)],
        ink: Ink::Ink,
    };
    let buf = render(&page_frame(vec![triangle], LiveValues::default()));
    near(pixel(&buf, 110, 60), bgra(p.ink), 2, "inside the triangle");
    near(
        pixel(&buf, 101, 45),
        bgra(p.ink),
        2,
        "near the top-left corner",
    );
    assert_ne!(
        pixel(&buf, 135, 45),
        bgra(p.ink),
        "outside the slanted edge"
    );
    assert_ne!(pixel(&buf, 95, 60), bgra(p.ink), "left of the base");
}

fn seek(playing: bool, ink: Ink) -> (Node, LiveValues) {
    let stamp = SystemTime::UNIX_EPOCH + Duration::from_secs(1000);
    let node = Node::Live(LiveNode::Seek {
        rect: Rect {
            x: 20.0,
            y: 95.0,
            w: 200.0,
            h: 10.0,
        },
        timeline: SeekTimeline {
            position_s: 60.0,
            duration_s: 120.0,
            stamp,
            playing,
        },
        ink,
    });
    let live = LiveValues {
        now: stamp,
        ..LiveValues::default()
    };
    (node, live)
}

#[test]
fn seek_head_takes_the_ink_of_its_node() {
    // Core decides the ink (flow while playing); the renderer only maps it.
    let p = Theme::Gece.palette();
    let (node, live) = seek(true, Ink::Flow);
    let buf = render(&page_frame(vec![node], live));
    near(pixel(&buf, 120, 100), bgra(p.flow), 2, "flow head");
    let (node, live) = seek(true, Ink::Ink);
    let buf = render(&page_frame(vec![node], live));
    near(
        pixel(&buf, 120, 100),
        bgra(p.ink),
        2,
        "ink head while playing",
    );
}

#[test]
fn trace_sits_on_a_faint_track_baseline() {
    // Mock `trace()`: a faint-track line under the samples, the samples in faint ink, a 14 DIP
    // swing above the baseline 1 DIP inside the 16 DIP slot.
    let p = Theme::Gece.palette();
    let trace = Node::Live(LiveNode::Trace {
        rect: Rect {
            x: 100.0,
            y: 40.5,
            w: 64.0,
            h: 16.0,
        },
        rail: LiveRail::Strip,
        index: 0,
    });
    let live = LiveValues {
        traces: vec![TraceSamples(vec![1.0; 9])],
        ..LiveValues::default()
    };
    let buf = render(&page_frame(vec![trace], live));
    near(pixel(&buf, 130, 55), bgra(p.faint_track), 2, "baseline");
    near(pixel(&buf, 130, 41), bgra(p.faint), 2, "full-scale samples");
}
