#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
//! Offscreen painter tests on WARP: no window, CI-safe.

use plith_core::ambient::LiveValues;
use plith_core::draw::PageContent;
use plith_core::geometry::IslandShape;
use plith_core::page::{PageLayer, PageLayers};
use plith_core::shapes::{PLACEHOLDER, REST};
use plith_render::paint::PaintFrame;
use plith_render::render_offscreen;
use std::sync::Arc;

/// The 480 x 180 shape these pixel checks were written for.
const SHAPE: IslandShape = IslandShape {
    width: 480.0,
    height: 180.0,
    radius: 40.0,
};

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

fn pixel(buf: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * width + x) * 4) as usize;
    [buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]
}

#[test]
fn island_pixels_are_opaque_black_and_outside_is_transparent() {
    let shape = SHAPE;
    let origin = (10.0, 10.0);
    let buf = render_offscreen(520, 220, 1.0, &frame(shape, origin, &[])).unwrap();
    assert_eq!(buf.len(), 520 * 220 * 4);
    let centre = pixel(&buf, 520, 250, 100);
    assert_eq!(centre, [0, 0, 0, 255]);
    assert_eq!(pixel(&buf, 520, 0, 0)[3], 0);
    // The top corners are square: the top-left pixel is opaque.
    assert_eq!(pixel(&buf, 520, 10, 10), [0, 0, 0, 255]);
    assert_eq!(pixel(&buf, 520, 489, 10), [0, 0, 0, 255]);
    // Bottom-left arc of r40 at origin (10, 10), bottom at 190: circle centre (50, 150). A pixel
    // d px diagonally in from the bounding corner is (40 - d) * sqrt(2) from the circle centre;
    // it is inside only when d > 40 * (1 - 1 / sqrt(2)) = 11.7.
    // d = 8: pixel (18, 181), centre (18.5, 181.5), distance 44.5 > 40: transparent.
    // d = 16: pixel (26, 173), centre (26.5, 173.5), distance 33.2 < 40: opaque.
    // A radius near 20 would make d = 8 opaque, so this pins the radius to about 40.
    assert_eq!(pixel(&buf, 520, 18, 181)[3], 0);
    assert_eq!(pixel(&buf, 520, 26, 173)[3], 255);
}

#[test]
fn scale_1_5_draws_the_same_shape_larger() {
    let shape = SHAPE;
    let origin = (10.0, 10.0);
    let buf = render_offscreen(780, 330, 1.5, &frame(shape, origin, &[])).unwrap();
    // Island spans px x 15..735, y 15..285 at scale 1.5.
    assert_eq!(pixel(&buf, 780, 375, 150), [0, 0, 0, 255]);
    assert_eq!(pixel(&buf, 780, 730, 150)[3], 255);
    assert_eq!(pixel(&buf, 780, 745, 150)[3], 0);
    assert_eq!(pixel(&buf, 780, 375, 280)[3], 255);
    assert_eq!(pixel(&buf, 780, 375, 295)[3], 0);
    // Square top-left corner, round bottom-left corner (d = 8 DIP = 12 px from the corner).
    assert_eq!(pixel(&buf, 780, 15, 15)[3], 255);
    assert_eq!(pixel(&buf, 780, 22, 277)[3], 0);
}

#[test]
fn page_placeholder_is_lighter_than_the_fill() {
    let shape = SHAPE;
    let origin = (10.0, 10.0);
    let layer = PageLayer {
        page: 2,
        offset: 0.0,
        opacity: 1.0,
    };
    let buf = render_offscreen(520, 220, 1.0, &frame(shape, origin, &[layer])).unwrap();
    let cx = 10.0 + shape.width / 2.0;
    let cy = 10.0 + PLACEHOLDER.top + PLACEHOLDER.height / 2.0;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let p = pixel(&buf, 520, cx as u32, cy as u32);
    assert!(p[0] > 10 && p[1] > 10 && p[2] > 10, "bar pixel {p:?}");
    assert_eq!(p[3], 255);
}

/// A placeholder pushed 240 DIP right of centre: it spans x 430..550 DIP, the island ends at 490.
fn bleeding_frame() -> PaintFrame {
    let layer = PageLayer {
        page: 2,
        offset: 240.0,
        opacity: 1.0,
    };
    frame(SHAPE, (10.0, 10.0), &[layer])
}

fn check_clip(scale: f64) {
    let w = (700.0 * scale) as u32;
    let h = (220.0 * scale) as u32;
    let at = |dip: f64| (dip * scale) as u32;
    let clipped = render_offscreen(w, h, scale, &bleeding_frame()).unwrap();
    // Inside the island the bar is lighter than the fill.
    assert!(pixel(&clipped, w, at(450.0), at(40.0))[0] > 10);
    // Outside the island (x = 520 DIP) the clip keeps alpha 0.
    assert_eq!(pixel(&clipped, w, at(520.0), at(40.0))[3], 0);
}

#[test]
fn clip_keeps_a_crossing_placeholder_inside_the_island() {
    check_clip(1.0);
}

#[test]
fn clip_keeps_a_crossing_placeholder_inside_the_island_at_scale_1_5() {
    check_clip(1.5);
}

#[test]
fn resting_island_with_hidden_content_shows_nothing_but_the_fill() {
    let (w, h) = (260_u32, 80_u32);
    let origin = (30.0, 10.0);
    let layer = PageLayer {
        page: 0,
        offset: 0.0,
        opacity: 1.0,
    };
    let mut f = frame(REST, origin, &[layer]);
    f.content_opacity = 0.0;
    let buf = render_offscreen(w, h, 1.0, &f).unwrap();
    let bottom = (origin.1 + REST.height) as u32;
    for y in 0..h {
        for x in 0..w {
            let p = pixel(&buf, w, x, y);
            assert_eq!(&p[..3], &[0, 0, 0], "lighter than the fill at ({x}, {y})");
            if y > bottom {
                assert_eq!(p[3], 0, "drawn outside the island at ({x}, {y})");
            }
        }
    }
    // The same frame with visible content does show the bar, so the test can fail.
    f.content_opacity = 1.0;
    let lit = render_offscreen(w, h, 1.0, &f).unwrap();
    let cy = (origin.1 + PLACEHOLDER.top + 4.0) as u32;
    assert!(pixel(&lit, w, 130, cy)[0] > 10);
}
