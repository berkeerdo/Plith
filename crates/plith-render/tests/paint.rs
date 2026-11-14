#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
//! Offscreen painter tests on WARP: no window, CI-safe.

use plith_core::geometry::IslandShape;
use plith_core::page::PageLayer;
use plith_core::shapes::{PAGES, PLACEHOLDER};
use plith_render::paint::PaintFrame;
use plith_render::render_offscreen;

fn frame(shape: IslandShape, origin: (f64, f64), pages: Vec<PageLayer>) -> PaintFrame {
    PaintFrame {
        shape,
        origin,
        pages,
        clip: false,
    }
}

fn pixel(buf: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * width + x) * 4) as usize;
    [buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]
}

#[test]
fn island_pixels_are_opaque_black_and_outside_is_transparent() {
    let shape = PAGES[2];
    let origin = (10.0, 10.0);
    let buf = render_offscreen(520, 220, 1.0, &frame(shape, origin, vec![])).unwrap();
    assert_eq!(buf.len(), 520 * 220 * 4);
    let centre = pixel(&buf, 520, 250, 100);
    assert_eq!(centre, [0, 0, 0, 255]);
    assert_eq!(pixel(&buf, 520, 0, 0)[3], 0);
    // Corner arc of r40 at origin (10, 10): circle centre (50, 50). A pixel d px diagonally in
    // from the bounding corner (10 + d) is (40 - d) * sqrt(2) from the circle centre;
    // it is inside only when d > 40 * (1 - 1 / sqrt(2)) = 11.7.
    // d = 8: pixel (18, 18), centre (18.5, 18.5), distance 44.5 > 40: transparent.
    // d = 16: pixel (26, 26), centre (26.5, 26.5), distance 33.2 < 40: opaque.
    // A radius near 20 would make d = 8 opaque, so this pins the radius to about 40.
    assert_eq!(pixel(&buf, 520, 18, 18)[3], 0);
    assert_eq!(pixel(&buf, 520, 26, 26)[3], 255);
}

#[test]
fn scale_1_5_draws_the_same_shape_larger() {
    let shape = PAGES[2];
    let origin = (10.0, 10.0);
    let buf = render_offscreen(780, 330, 1.5, &frame(shape, origin, vec![])).unwrap();
    // Island spans px x 15..735, y 15..285 at scale 1.5.
    assert_eq!(pixel(&buf, 780, 375, 150), [0, 0, 0, 255]);
    assert_eq!(pixel(&buf, 780, 730, 150)[3], 255);
    assert_eq!(pixel(&buf, 780, 745, 150)[3], 0);
    assert_eq!(pixel(&buf, 780, 375, 280)[3], 255);
    assert_eq!(pixel(&buf, 780, 375, 295)[3], 0);
    assert_eq!(pixel(&buf, 780, 18, 18)[3], 0);
}

#[test]
fn page_placeholder_is_lighter_than_the_fill() {
    let shape = PAGES[2];
    let origin = (10.0, 10.0);
    let layer = PageLayer {
        page: 2,
        offset: 0.0,
        opacity: 1.0,
    };
    let buf = render_offscreen(520, 220, 1.0, &frame(shape, origin, vec![layer])).unwrap();
    let cx = 10.0 + shape.width / 2.0;
    let cy = 10.0 + PLACEHOLDER.top + PLACEHOLDER.height / 2.0;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let p = pixel(&buf, 520, cx as u32, cy as u32);
    assert!(p[0] > 10 && p[1] > 10 && p[2] > 10, "bar pixel {p:?}");
    assert_eq!(p[3], 255);
}

/// A placeholder pushed 240 DIP right of centre: it spans x 430..550 DIP, the island ends at 490.
fn bleeding_frame(clip: bool) -> PaintFrame {
    let layer = PageLayer {
        page: 2,
        offset: 240.0,
        opacity: 1.0,
    };
    let mut f = frame(PAGES[2], (10.0, 10.0), vec![layer]);
    f.clip = clip;
    f
}

fn check_clip(scale: f64) {
    let w = (700.0 * scale) as u32;
    let h = (220.0 * scale) as u32;
    let at = |dip: f64| (dip * scale) as u32;
    let clipped = render_offscreen(w, h, scale, &bleeding_frame(true)).unwrap();
    let open = render_offscreen(w, h, scale, &bleeding_frame(false)).unwrap();
    // Inside the island the bar is lighter than the fill in both cases.
    assert!(pixel(&clipped, w, at(450.0), at(40.0))[0] > 10);
    // Outside the island (x = 520 DIP) the clip keeps alpha 0, the open frame bleeds.
    assert_eq!(pixel(&clipped, w, at(520.0), at(40.0))[3], 0);
    assert!(pixel(&open, w, at(520.0), at(40.0))[3] > 0);
}

#[test]
fn clip_keeps_a_crossing_placeholder_inside_the_island() {
    check_clip(1.0);
}

#[test]
fn clip_keeps_a_crossing_placeholder_inside_the_island_at_scale_1_5() {
    check_clip(1.5);
}
