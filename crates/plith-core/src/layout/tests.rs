use super::*;
use crate::shapes::{OVERSHOOT, PAGES, REST, SURFACE_MARGIN, TOP_GAP};

const MONITOR: RectPx = RectPx {
    x: 0,
    y: 0,
    width: 2560,
    height: 1440,
};

/// The surface size in DIP for the largest page, derived from the shape constants.
fn surface_dip() -> (f64, f64) {
    let max_w = PAGES.iter().fold(0.0_f64, |a, p| a.max(p.width));
    let max_h = PAGES.iter().fold(0.0_f64, |a, p| a.max(p.height));
    (
        max_w * OVERSHOOT + 2.0 * SURFACE_MARGIN,
        TOP_GAP + max_h * OVERSHOOT + SURFACE_MARGIN,
    )
}

/// Whole pixels for `dip` at `scale`, rounded up.
#[allow(clippy::cast_possible_truncation)]
fn px(dip: f64, scale: f64) -> i32 {
    (dip * scale - 1e-9).ceil() as i32
}

/// The same, rounded up to an even count (the surface width is even).
fn even_px(dip: f64, scale: f64) -> i32 {
    px(dip, scale) + px(dip, scale) % 2
}

#[test]
fn surface_is_even_width_and_centred_on_the_monitor_centre_pixel() {
    let l = SurfaceLayout::new(MONITOR, 0, 1.0);
    assert_eq!(l.surface.width % 2, 0);
    assert_eq!(l.surface.x + l.surface.width / 2, 1280);
    assert_eq!(l.surface.y, 0);
    let offset = RectPx {
        x: -1920,
        y: 0,
        width: 1920,
        height: 1080,
    };
    let l = SurfaceLayout::new(offset, 40, 1.0);
    assert_eq!(l.surface.x + l.surface.width / 2, -1920 + 960);
    assert_eq!(l.surface.y, 40);
}

#[test]
fn surface_fits_the_largest_page_with_overshoot_and_margin() {
    let l = SurfaceLayout::new(MONITOR, 0, 1.0);
    let (dip_w, dip_h) = surface_dip();
    assert!(f64::from(l.surface.width) >= dip_w - 1e-6);
    assert!(f64::from(l.surface.height) >= dip_h - 1e-6);
    assert_eq!(l.surface.width, even_px(dip_w, 1.0));
    assert_eq!(l.surface.height, px(dip_h, 1.0));
}

#[test]
fn layout_scales_with_dpi() {
    let l = SurfaceLayout::new(MONITOR, 0, 1.5);
    let (dip_w, dip_h) = surface_dip();
    assert_eq!(l.surface.width, even_px(dip_w, 1.5));
    assert_eq!(l.surface.height, px(dip_h, 1.5));
    assert_eq!(l.surface.width % 2, 0);
}

#[test]
fn island_origin_is_centred_with_top_gap() {
    let l = SurfaceLayout::new(MONITOR, 0, 1.0);
    let (x, y) = l.island_origin(REST);
    let (dip_w, _) = surface_dip();
    let surface_w = f64::from(even_px(dip_w, 1.0));
    assert!((x - (surface_w - REST.width) / 2.0).abs() < 1e-9);
    assert!((y - TOP_GAP).abs() < 1e-9);
}
