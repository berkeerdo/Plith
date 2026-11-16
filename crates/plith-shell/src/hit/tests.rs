use super::*;
use plith_core::layout::RectPx;
use plith_core::shapes::TOP_GAP;

fn layout(width: i32, scale: f64) -> SurfaceLayout {
    SurfaceLayout {
        surface: RectPx {
            x: 0,
            y: 0,
            width,
            height: 600,
        },
        scale,
    }
}

const SHAPE: IslandShape = IslandShape {
    width: 100.0,
    height: 40.0,
    radius: 20.0,
};

#[test]
fn rect_is_centred_where_island_origin_says() {
    let region = region_px(SHAPE, &layout(400, 1.0), 0);
    #[allow(clippy::cast_possible_truncation)]
    let top = TOP_GAP.round() as i32;
    assert_eq!(region.left, 150);
    assert_eq!(region.right, 250);
    assert_eq!(region.top, top);
    assert_eq!(region.bottom, top + 40);
    assert_eq!(region.ellipse, 40);
}

#[test]
fn outset_grows_sides_and_bottom_by_one_pixel_but_not_the_top() {
    let plain = region_px(SHAPE, &layout(400, 1.0), 0);
    let grown = region_px(SHAPE, &layout(400, 1.0), 1);
    assert_eq!(grown.left, plain.left - 1);
    assert_eq!(grown.top, plain.top);
    assert_eq!(grown.right, plain.right + 1);
    assert_eq!(grown.bottom, plain.bottom + 1);
    assert_eq!(grown.ellipse, plain.ellipse + 2);
}

#[test]
fn scale_multiplies_the_rect() {
    let region = region_px(SHAPE, &layout(600, 1.5), 0);
    // Surface is 400 DIP wide, so the island starts at 150 DIP = 225 px.
    assert_eq!(region.left, 225);
    assert_eq!(region.right, 375);
    assert_eq!(region.bottom - region.top, 60);
    assert_eq!(region.ellipse, 60);
}

#[test]
fn radius_is_clamped_to_half_the_short_side() {
    let shape = IslandShape {
        radius: 500.0,
        ..SHAPE
    };
    let region = region_px(shape, &layout(400, 1.0), 0);
    assert_eq!(region.ellipse, 40);
}

#[test]
fn region_top_corners_square() {
    let region = region_px(SHAPE, &layout(400, 1.0), 3);
    // Flush at the top: no outset above the island.
    assert_eq!(region.top, 0);
    // The square band covers the top half of the outset rect.
    assert_eq!(
        region.square_bottom,
        region.top + (region.bottom - region.top) / 2
    );
    assert!(region.square_bottom > region.top);
    // Sides and bottom are still 3 px outside the drawn shape.
    assert_eq!(region.left, 150 - 3);
    assert_eq!(region.right, 250 + 3);
    assert_eq!(region.bottom, 40 + 3);
}
