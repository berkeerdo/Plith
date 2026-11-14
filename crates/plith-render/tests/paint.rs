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
    // Inside the bounding box but outside the corner arc.
    assert_eq!(pixel(&buf, 520, 12, 12)[3], 0);
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

#[test]
fn clipped_frame_draws_the_placeholder_inside_the_island() {
    let shape = PAGES[2];
    let layer = PageLayer {
        page: 2,
        offset: 0.0,
        opacity: 1.0,
    };
    let mut clipped = frame(shape, (10.0, 10.0), vec![layer]);
    clipped.clip = true;
    let buf = render_offscreen(520, 220, 1.0, &clipped).unwrap();
    let bar = pixel(&buf, 520, 250, 40);
    assert!(bar[0] > 10 && bar[3] == 255, "bar pixel {bar:?}");
    assert_eq!(pixel(&buf, 520, 12, 12)[3], 0);
}
