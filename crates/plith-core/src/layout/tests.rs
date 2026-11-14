use super::*;
use crate::shapes::REST;

const MONITOR: RectPx = RectPx {
    x: 0,
    y: 0,
    width: 2560,
    height: 1440,
};

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
    assert!(f64::from(l.surface.width) >= 620.0 * 1.05 + 40.0 - 1e-6);
    assert!(f64::from(l.surface.height) >= 8.0 + 320.0 * 1.05 + 20.0 - 1e-6);
    assert_eq!(l.surface.width, 692);
    assert_eq!(l.surface.height, 364);
}

#[test]
fn layout_scales_with_dpi() {
    let l = SurfaceLayout::new(MONITOR, 0, 1.5);
    assert_eq!(l.surface.width, 1038);
    assert_eq!(l.surface.height, 546);
    assert_eq!(l.surface.width % 2, 0);
}

#[test]
fn island_origin_is_centred_with_top_gap() {
    let l = SurfaceLayout::new(MONITOR, 0, 1.0);
    let (x, y) = l.island_origin(REST);
    assert!((x - (692.0 - 200.0) / 2.0).abs() < 1e-9);
    assert!((y - 8.0).abs() < 1e-9);
}
