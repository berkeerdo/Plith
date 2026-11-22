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

#[test]
fn an_unchanged_covered_shape_is_not_applied_again() {
    use crate::window::state::Inner;
    use plith_core::geometry::cover;
    use plith_core::page::PageLayers;
    use plith_core::scene::{ShapeCell, ShapeFrame};
    use plith_core::shapes::{PAGES, REST};
    use std::sync::Arc;
    let cell = Arc::new(ShapeCell::default());
    let inner = Inner::new(
        crate::window::display::test_display(layout(1920, 1.0)),
        Arc::clone(&cell),
        Box::new(|_| {}),
        Box::new(|_| {}),
    );
    let frame = |generation, pages| ShapeFrame {
        shape: PAGES[0],
        previous: PAGES[0],
        generation,
        pages,
    };
    // The window does not exist, so the region call fails and is retried on the next post.
    cell.publish(frame(1, PageLayers::empty()));
    apply::apply_latest(&inner);
    apply::apply_latest(&inner);
    assert_eq!(inner.stats.get().count, 2, "a failed apply is retried");
    // Once applied, a page-only frame at the same covered shape skips SetWindowRgn.
    inner.applied.set(Some(cover(PAGES[0], PAGES[0])));
    cell.publish(frame(2, PageLayers::empty()));
    apply::apply_latest(&inner);
    assert_eq!(
        inner.stats.get().count,
        2,
        "same covered shape: no region call"
    );
    cell.publish(ShapeFrame {
        previous: REST,
        ..frame(3, PageLayers::empty())
    });
    apply::apply_latest(&inner);
    assert_eq!(inner.stats.get().count, 3, "a new covered shape is applied");
}
