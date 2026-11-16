#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
//! Offscreen painter tests on WARP: no window, CI-safe.

use plith_core::ambient::{LiveValues, TraceSamples};
use plith_core::draw::{
    Align, ArtId, ArtPixels, DisplayList, Ink, LiveNode, LiveRail, Node, PageContent, Rect,
    SeekTimeline, StationMark, TextStyle, Trim,
};
use plith_core::geometry::IslandShape;
use plith_core::page::{PageLayer, PageLayers};
use plith_core::shapes::REST;
use plith_core::theme::{Palette, Theme};
use plith_render::offscreen::Offscreen;
use plith_render::paint::PaintFrame;
use plith_render::{TextSystem, render_offscreen};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

/// The 480 x 180 shape these pixel checks were written for.
const SHAPE: IslandShape = IslandShape {
    width: 480.0,
    height: 180.0,
    radius: 40.0,
};
const ORIGIN: (f64, f64) = (10.0, 10.0);
const W: u32 = 520;
const H: u32 = 220;

fn layer(page: usize, offset: f64, opacity: f64) -> PageLayer {
    PageLayer {
        page,
        offset,
        opacity,
    }
}

fn frame(shape: IslandShape, origin: (f64, f64), pages: &[PageLayer]) -> PaintFrame {
    PaintFrame {
        shape,
        origin,
        pages: PageLayers::of(pages),
        content_opacity: 1.0,
        content: Arc::new(PageContent::empty()),
        content_gen: 0,
        live: LiveValues::default(),
    }
}

/// A page list the size of `SHAPE`.
fn list(nodes: Vec<Node>) -> Arc<DisplayList> {
    Arc::new(DisplayList {
        nodes,
        size: (SHAPE.width, SHAPE.height),
    })
}

/// A frame of `SHAPE` at `ORIGIN` resting on page 0, which holds `nodes`.
fn page_frame(nodes: Vec<Node>) -> PaintFrame {
    let mut content = PageContent::empty();
    content.pages[0] = list(nodes);
    let mut f = frame(SHAPE, ORIGIN, &[layer(0, 0.0, 1.0)]);
    f.content = Arc::new(content);
    f
}

fn render(f: &PaintFrame) -> Vec<u8> {
    render_scaled(W, H, 1.0, f)
}

fn render_scaled(w: u32, h: u32, scale: f64, f: &PaintFrame) -> Vec<u8> {
    let text = TextSystem::new().expect("text system");
    render_offscreen(w, h, scale, f, Palette::default(), text).unwrap()
}

fn pixel(buf: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * width + x) * 4) as usize;
    [buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]
}

/// A palette colour as the opaque BGRA bytes it renders to.
fn bgra(c: [f32; 4]) -> [u8; 4] {
    let b = |v: f32| (v * 255.0).round() as u8;
    [b(c[2]), b(c[1]), b(c[0]), 255]
}

fn near(got: [u8; 4], want: [u8; 4], tol: u8, what: &str) {
    let ok = got.iter().zip(want).all(|(g, w)| g.abs_diff(w) <= tol);
    assert!(ok, "{what}: got {got:?}, want {want:?} within {tol}");
}

/// The island with no pages: the background a page is drawn over.
fn empty() -> Vec<u8> {
    render(&frame(SHAPE, ORIGIN, &[]))
}

fn gece() -> Palette {
    Theme::Gece.palette()
}

#[test]
fn island_fill_is_a_floor_gradient_with_a_border_except_on_top() {
    let p = gece();
    let buf = render(&frame(SHAPE, ORIGIN, &[]));
    assert_eq!(buf.len(), (W * H * 4) as usize);
    assert_eq!(pixel(&buf, W, 0, 0)[3], 0);
    near(
        pixel(&buf, W, 250, 10),
        bgra(p.floor),
        1,
        "top centre is floor",
    );
    near(
        pixel(&buf, W, 250, 188),
        bgra(p.floor_high),
        1,
        "bottom is floor high",
    );
    near(pixel(&buf, W, 10, 100), bgra(p.border), 1, "left border");
    near(pixel(&buf, W, 489, 100), bgra(p.border), 1, "right border");
    near(pixel(&buf, W, 250, 189), bgra(p.border), 1, "bottom border");
    assert!(pixel(&buf, W, 11, 100)[2] < 0x18, "inside the border");
    // No border along the top edge; the sides run up to it.
    near(pixel(&buf, W, 100, 10), bgra(p.floor), 1, "no top border");
    near(
        pixel(&buf, W, 10, 10),
        bgra(p.border),
        1,
        "left side reaches the top",
    );
}

#[test]
fn island_has_a_square_top_and_rounded_bottom() {
    let buf = render(&frame(SHAPE, ORIGIN, &[]));
    assert_eq!(pixel(&buf, W, 10, 10)[3], 255);
    assert_eq!(pixel(&buf, W, 489, 10)[3], 255);
    // Bottom-left arc of r40: circle centre (50, 150). d = 8 from the corner is outside,
    // d = 16 inside (see the R1 note: this pins the radius to about 40).
    assert_eq!(pixel(&buf, W, 18, 181)[3], 0);
    assert_eq!(pixel(&buf, W, 26, 173)[3], 255);
}

#[test]
fn scale_1_5_draws_the_same_shape_larger() {
    let buf = render_scaled(780, 330, 1.5, &frame(SHAPE, ORIGIN, &[]));
    // Island spans px x 15..735, y 15..285 at scale 1.5.
    assert_eq!(pixel(&buf, 780, 375, 150)[3], 255);
    assert_eq!(pixel(&buf, 780, 730, 150)[3], 255);
    assert_eq!(pixel(&buf, 780, 745, 150)[3], 0);
    assert_eq!(pixel(&buf, 780, 375, 280)[3], 255);
    assert_eq!(pixel(&buf, 780, 375, 295)[3], 0);
    assert_eq!(pixel(&buf, 780, 15, 15)[3], 255);
    assert_eq!(pixel(&buf, 780, 22, 277)[3], 0);
}

#[test]
fn station_filled_and_hollow_pixels() {
    let p = gece();
    let track = Node::Stroke {
        points: vec![(50.0, 60.5), (110.0, 60.5)],
        width: 1.0,
        ink: Ink::Track,
        dashed: false,
    };
    let station = |x: f64, mark: StationMark| Node::Station {
        x,
        y: 60.5,
        mark,
        ink: Ink::Ink,
    };
    let f = page_frame(vec![
        track,
        station(30.5, StationMark::Filled),
        station(80.5, StationMark::Hollow),
    ]);
    let buf = render(&f);
    // Page (0, 0) is pixel (10, 10): the filled square spans px 37..44, the hollow 87..94.
    near(pixel(&buf, W, 40, 70), bgra(p.ink), 2, "filled centre");
    near(pixel(&buf, W, 37, 67), bgra(p.ink), 2, "filled corner");
    near(
        pixel(&buf, W, 90, 70),
        bgra(p.floor_high),
        2,
        "hollow centre hides the line",
    );
    near(pixel(&buf, W, 87, 70), bgra(p.ink), 2, "hollow left edge");
    near(pixel(&buf, W, 90, 67), bgra(p.ink), 2, "hollow top edge");
    near(
        pixel(&buf, W, 88, 70),
        bgra(p.floor_high),
        2,
        "1 px ring only",
    );
    near(
        pixel(&buf, W, 70, 70),
        bgra(p.track),
        2,
        "the line outside the stations",
    );
}

#[test]
fn lit_route_is_two_px_flow() {
    let p = gece();
    let f = page_frame(vec![Node::Stroke {
        points: vec![(20.0, 80.0), (100.0, 80.0), (120.0, 100.0), (200.0, 100.0)],
        width: 2.0,
        ink: Ink::Flow,
        dashed: false,
    }]);
    let buf = render(&f);
    let bg = empty();
    near(pixel(&buf, W, 60, 89), bgra(p.flow), 2, "upper row");
    near(pixel(&buf, W, 60, 90), bgra(p.flow), 2, "lower row");
    near(pixel(&buf, W, 60, 88), pixel(&bg, W, 60, 88), 1, "above");
    near(pixel(&buf, W, 60, 91), pixel(&bg, W, 60, 91), 1, "below");
    near(
        pixel(&buf, W, 170, 110),
        bgra(p.flow),
        2,
        "after the 45 degree turn",
    );
}

#[test]
fn dashed_stroke_has_gaps() {
    let f = page_frame(vec![Node::Stroke {
        points: vec![(20.0, 120.5), (90.0, 120.5)],
        width: 1.0,
        ink: Ink::Flow,
        dashed: true,
    }]);
    let buf = render(&f);
    let row: Vec<[u8; 4]> = (32..98).map(|x| pixel(&buf, W, x, 130)).collect();
    let lit = row.iter().filter(|px| px[1] > 0xC0).count();
    let dark = row.iter().filter(|px| px[1] < 0x20).count();
    assert!(
        lit > 25 && dark > 15,
        "dash 4 gap 3: {lit} lit, {dark} dark"
    );
}

/// True when the pixel is much brighter than the Gece floor (text ink).
fn bright(px: [u8; 4]) -> bool {
    px[2] > 0x80
}

fn lit_columns(buf: &[u8], x0: u32, x1: u32) -> usize {
    (x0..x1)
        .filter(|&x| (50..74).any(|y| bright(pixel(buf, W, x, y))))
        .count()
}

#[test]
fn text_is_drawn_in_ink_and_aligned() {
    let text = |align: Align| Node::Text {
        rect: Rect {
            x: 20.0,
            y: 40.0,
            w: 300.0,
            h: 24.0,
        },
        text: Arc::from("MMMM"),
        style: TextStyle::Title,
        ink: Ink::Ink,
        align,
        trim: Trim::End,
    };
    let start = render(&page_frame(vec![text(Align::Start)]));
    assert!(lit_columns(&start, 30, 130) > 20, "start: ink at the left");
    assert_eq!(
        lit_columns(&start, 200, 330),
        0,
        "start: nothing at the right"
    );
    let end = render(&page_frame(vec![text(Align::End)]));
    assert_eq!(lit_columns(&end, 30, 200), 0, "end: nothing at the left");
    assert!(lit_columns(&end, 230, 330) > 20, "end: ink at the right");
}

#[test]
fn page_bitmap_reused_across_frames() {
    let text = TextSystem::new().unwrap();
    let mut off = Offscreen::new(W, H, 1.0, Palette::default(), text).unwrap();
    let mut f = page_frame(vec![Node::Fill {
        rect: Rect {
            x: 20.0,
            y: 40.0,
            w: 100.0,
            h: 20.0,
        },
        radius: 0.0,
        ink: Ink::Ink,
    }]);
    let first = off.render(&f).unwrap();
    assert_eq!(off.page_rebuilds(), 1);
    f.live = LiveValues {
        strips: vec![[0.5, 0.5]],
        ..LiveValues::default()
    };
    f.pages = PageLayers::of(&[layer(0, 3.0, 0.8)]);
    let _ = off.render(&f).unwrap();
    f.pages = PageLayers::of(&[layer(0, 0.0, 1.0)]);
    let again = off.render(&f).unwrap();
    assert_eq!(off.page_rebuilds(), 1, "same list, same scale and palette");
    assert_eq!(off.island_builds(), 1, "same shape and origin");
    assert_eq!(first, again);
    // An equal list in a new Arc is a new list.
    let mut content = (*f.content).clone();
    content.pages[0] = Arc::new((*content.pages[0]).clone());
    f.content = Arc::new(content);
    let _ = off.render(&f).unwrap();
    assert_eq!(off.page_rebuilds(), 2);
    off.set_palette(Theme::York.palette());
    let york = off.render(&f).unwrap();
    assert_eq!(off.page_rebuilds(), 3);
    assert_eq!(off.island_builds(), 1, "a palette change keeps the paths");
    near(
        pixel(&york, W, 60, 60),
        bgra(Theme::York.palette().ink),
        2,
        "redrawn in the new ink",
    );
}

#[test]
fn outgoing_page_fades_with_opacity() {
    let fill = Node::Fill {
        rect: Rect {
            x: 20.0,
            y: 40.0,
            w: 100.0,
            h: 20.0,
        },
        radius: 0.0,
        ink: Ink::Ink,
    };
    let mut content = PageContent::empty();
    content.pages[0] = list(vec![fill]);
    let mut f = frame(SHAPE, ORIGIN, &[layer(0, 0.0, 0.5), layer(1, 0.0, 0.5)]);
    f.content = Arc::new(content);
    let faded = render(&f);
    let bg = render(&frame(SHAPE, ORIGIN, &[]));
    let ink = bgra(gece().ink);
    let under = pixel(&bg, W, 60, 60);
    let mut want = [0u8; 4];
    for i in 0..3 {
        want[i] = f32::midpoint(f32::from(ink[i]), f32::from(under[i])).round() as u8;
    }
    want[3] = 255;
    near(pixel(&faded, W, 60, 60), want, 3, "half ink over the floor");
    // The content opacity multiplies in.
    f.content_opacity = 0.0;
    near(pixel(&render(&f), W, 60, 60), under, 1, "hidden content");
}

fn meter(rail: LiveRail, index: usize) -> Node {
    Node::Live(LiveNode::Meter {
        rect: Rect {
            x: 20.0,
            y: 40.0,
            w: 100.0,
            h: 6.0,
        },
        rail,
        index,
    })
}

#[test]
fn live_meter_draws_level() {
    let p = gece();
    let mut f = page_frame(vec![meter(LiveRail::Strip, 0), meter(LiveRail::Bus, 3)]);
    let bg = empty();
    let quiet = render(&f);
    near(
        pixel(&quiet, W, 40, 52),
        pixel(&bg, W, 40, 52),
        1,
        "no level, no meter",
    );
    f.live = LiveValues {
        strips: vec![[0.2, 0.5]],
        ..LiveValues::default()
    };
    let buf = render(&f);
    near(pixel(&buf, W, 40, 52), bgra(p.flow), 2, "inside the level");
    near(
        pixel(&buf, W, 75, 52),
        bgra(p.flow),
        2,
        "peak is the louder side",
    );
    near(
        pixel(&buf, W, 95, 52),
        pixel(&bg, W, 95, 52),
        1,
        "past the level",
    );
    // The incoming page's opacity reaches the live nodes.
    f.pages = PageLayers::of(&[layer(0, 0.0, 0.0)]);
    near(
        pixel(&render(&f), W, 40, 52),
        pixel(&bg, W, 40, 52),
        1,
        "faded with its page",
    );
}

#[test]
fn trace_and_seek_draw_from_live_values() {
    let p = gece();
    let stamp = SystemTime::UNIX_EPOCH + Duration::from_secs(1000);
    let seek = Node::Live(LiveNode::Seek {
        rect: Rect {
            x: 20.0,
            y: 100.0,
            w: 200.0,
            h: 10.0,
        },
        timeline: SeekTimeline {
            position_s: 30.0,
            duration_s: 120.0,
            stamp,
            playing: true,
        },
    });
    let trace = Node::Live(LiveNode::Trace {
        rect: Rect {
            x: 260.0,
            y: 130.0,
            w: 64.0,
            h: 21.0,
        },
        rail: LiveRail::Bus,
        index: 0,
    });
    let mut f = page_frame(vec![seek, trace]);
    f.live = LiveValues {
        traces: vec![TraceSamples(vec![0.5; 5])],
        now: stamp + Duration::from_secs(30),
        ..LiveValues::default()
    };
    let buf = render(&f);
    let bg = empty();
    // Played half: a 2 px line at y 105 (rows 114, 115 on screen) from x 20 to 120.
    near(pixel(&buf, W, 70, 114), bgra(p.flow), 2, "played part");
    near(
        pixel(&buf, W, 70, 115),
        bgra(p.flow),
        2,
        "played part, second row",
    );
    near(
        pixel(&buf, W, 190, 114),
        pixel(&bg, W, 190, 114),
        1,
        "unplayed part",
    );
    near(pixel(&buf, W, 130, 114), bgra(p.ink), 2, "head station");
    // The trace: five samples at 0.5 lie on y 140.5, row 150 on screen.
    near(pixel(&buf, W, 300, 150), bgra(p.track), 2, "trace line");
}

#[test]
fn image_draws_its_art_and_missing_art_draws_nothing() {
    let image = |id: u64| Node::Image {
        rect: Rect {
            x: 300.0,
            y: 40.0,
            w: 20.0,
            h: 20.0,
        },
        art: ArtId(id),
    };
    let mut content = PageContent::empty();
    content.pages[0] = list(vec![image(7)]);
    content.art = vec![Arc::new(ArtPixels {
        id: ArtId(7),
        width: 2,
        height: 2,
        bgra: Arc::from([0u8, 0, 255, 255].repeat(4)),
    })];
    let mut f = frame(SHAPE, ORIGIN, &[layer(0, 0.0, 1.0)]);
    f.content = Arc::new(content.clone());
    near(
        pixel(&render(&f), W, 320, 60),
        [0, 0, 255, 255],
        2,
        "red art",
    );
    content.pages[0] = list(vec![image(8)]);
    f.content = Arc::new(content);
    near(
        pixel(&render(&f), W, 320, 60),
        pixel(&empty(), W, 320, 60),
        1,
        "missing art",
    );
}

/// A page filling the whole island, pushed `offset` DIP right: it crosses the island edge.
fn crossing_frame(offset: f64) -> PaintFrame {
    let mut f = page_frame(vec![Node::Fill {
        rect: Rect {
            x: 0.0,
            y: 0.0,
            w: SHAPE.width,
            h: SHAPE.height,
        },
        radius: 0.0,
        ink: Ink::Ink,
    }]);
    f.pages = PageLayers::of(&[layer(0, offset, 1.0)]);
    f
}

fn check_clip(scale: f64) {
    let w = (700.0 * scale) as u32;
    let h = (220.0 * scale) as u32;
    let at = |dip: f64| (dip * scale) as u32;
    let clipped = render_scaled(w, h, scale, &crossing_frame(240.0));
    assert!(
        pixel(&clipped, w, at(450.0), at(40.0))[2] > 0x80,
        "page inside"
    );
    assert_eq!(
        pixel(&clipped, w, at(520.0), at(40.0))[3],
        0,
        "clipped outside"
    );
    let resting = render_scaled(w, h, scale, &crossing_frame(0.0));
    assert_eq!(
        pixel(&resting, w, at(18.0), at(181.0))[3],
        0,
        "rounded corner"
    );
}

#[test]
fn clip_keeps_a_crossing_page_inside_the_island() {
    check_clip(1.0);
}

#[test]
fn clip_keeps_a_crossing_page_inside_the_island_at_scale_1_5() {
    check_clip(1.5);
}

#[test]
fn collapsed_island_draws_the_pill_not_the_pages() {
    let p = gece();
    let (w, h) = (260_u32, 80_u32);
    let origin = (30.0, 10.0);
    let spot = Rect {
        x: 20.0,
        y: 10.0,
        w: 30.0,
        h: 10.0,
    };
    let mut content = PageContent::empty();
    content.pages[0] = list(vec![Node::Fill {
        rect: spot,
        radius: 0.0,
        ink: Ink::Ink,
    }]);
    content.pill = Arc::new(DisplayList {
        nodes: vec![Node::Fill {
            rect: spot,
            radius: 0.0,
            ink: Ink::Flow,
        }],
        size: (REST.width, REST.height),
    });
    let mut f = frame(REST, origin, &[layer(0, 0.0, 1.0)]);
    f.content = Arc::new(content);
    f.content_opacity = 0.0;
    let text = TextSystem::new().unwrap();
    let collapsed = render_offscreen(w, h, 1.0, &f, Palette::default(), text).unwrap();
    near(pixel(&collapsed, w, 65, 25), bgra(p.flow), 2, "pill drawn");
    f.content_opacity = 1.0;
    let text = TextSystem::new().unwrap();
    let open = render_offscreen(w, h, 1.0, &f, Palette::default(), text).unwrap();
    near(
        pixel(&open, w, 65, 25),
        bgra(p.ink),
        2,
        "page drawn, pill hidden",
    );
}

#[test]
fn pages_snap_to_device_pixels_at_fractional_scale() {
    // At 1.25 the origin 10 DIP is 12.5 px: unsnapped, a 1 px column would blur over two.
    let scale = 1.25;
    let (w, h) = (650_u32, 275_u32);
    let mut f = page_frame(vec![Node::Fill {
        rect: Rect {
            x: 8.0,
            y: 16.0,
            w: 0.8,
            h: 32.0,
        },
        radius: 0.0,
        ink: Ink::Ink,
    }]);
    f.pages = PageLayers::of(&[layer(0, 0.0, 1.0)]);
    let buf = render_scaled(w, h, scale, &f);
    let ink = bgra(gece().ink);
    let row: Vec<[u8; 4]> = (18..30).map(|x| pixel(&buf, w, x, 45)).collect();
    let full = row.iter().filter(|px| px[2].abs_diff(ink[2]) <= 2).count();
    let partial = row
        .iter()
        .filter(|px| px[2] > 0x30 && px[2].abs_diff(ink[2]) > 2)
        .count();
    assert_eq!((full, partial), (1, 0), "one sharp column: {row:?}");
}

#[test]
fn outgoing_page_keeps_its_live_nodes() {
    let p = gece();
    let mut content = PageContent::empty();
    content.pages[0] = list(vec![meter(LiveRail::Strip, 0)]);
    let mut f = frame(SHAPE, ORIGIN, &[layer(0, 0.0, 1.0), layer(1, 0.0, 1.0)]);
    f.content = Arc::new(content);
    f.live = LiveValues {
        strips: vec![[0.5, 0.5]],
        ..LiveValues::default()
    };
    near(
        pixel(&render(&f), W, 40, 52),
        bgra(p.flow),
        2,
        "outgoing meter",
    );
}
