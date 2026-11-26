#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
//! Per-pixel contrast of the built-in material packs (spec `2026-11-26-theme-worlds-design.md`,
//! section 5.3): the ink, muted, faint and flow colours must read on every pixel of the text
//! zone of the raster floor, at both file scales. Decoded with `plith-media` (WIC), as the app
//! does.

use plith_core::theme::builtin::{BUILTIN, BuiltinPack};
use plith_core::theme::colour::Rgb;
use plith_core::theme::pack::{Floor, FlowStyle, Pack, parse_pack};
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};

/// The least contrast each colour needs against every pixel (WCAG: body text 7, secondary text
/// 4.5, a large mark 3).
const MIN_INK: f64 = 7.0;
const MIN_MUTED: f64 = 4.5;
const MIN_FAINT: f64 = 3.0;
const MIN_FLOW: f64 = 3.0;

fn com() {
    // SAFETY: initializes COM for this test thread; a repeat call returns S_FALSE and is fine.
    let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
}

/// One decoded floor: its size and premultiplied BGRA rows.
struct Floored {
    width: u32,
    height: u32,
    bgra: std::sync::Arc<[u8]>,
}

fn decode(pack: &BuiltinPack, name: &str) -> Floored {
    com();
    let bytes = pack
        .files
        .iter()
        .find(|(file, _)| *file == name)
        .unwrap_or_else(|| panic!("no file {name}"))
        .1;
    let art = plith_media::art::decode_image(bytes, u32::MAX)
        .unwrap_or_else(|| panic!("{name} does not decode"));
    Floored {
        width: art.width,
        height: art.height,
        bgra: art.bgra,
    }
}

/// Every built-in pack whose open floor is a raster material, with its builtin entry.
fn material_packs() -> Vec<(Pack, &'static BuiltinPack)> {
    BUILTIN
        .iter()
        .map(|b| (parse_pack(b.manifest).expect("built-in parses"), b))
        .filter(|(pack, _)| matches!(pack.open.floor, Floor::Material { .. }))
        .collect()
}

/// Refuses a zone that is empty or leaves the `width` x `height` floor: a check over it would
/// pass without looking at the pixels it names.
fn check_rect(rect: (u32, u32, u32, u32), width: u32, height: u32) {
    let (x0, y0, x1, y1) = rect;
    assert!(
        x1 > x0 && y1 > y0 && x1 <= width && y1 <= height,
        "text zone {rect:?} is empty or outside the {width}x{height} floor"
    );
}

/// The pixel rectangle of the text zone at `scale` (1 or 2): `(x0, y0, x1, y1)`, exclusive end.
fn zone(pack: &Pack, scale: u32, floor: &Floored) -> (u32, u32, u32, u32) {
    let [left, top, wide, high] = pack.text_zone.expect("a material pack names its text zone");
    let s = scale as f32;
    let (x0, y0) = ((left * s) as u32, (top * s) as u32);
    let rect = (x0, y0, x0 + (wide * s) as u32, y0 + (high * s) as u32);
    check_rect(rect, floor.width, floor.height);
    rect
}

/// Every pixel of the text zone, after checking that exactly `w*scale * h*scale` were visited.
fn zone_pixels(pack: &Pack, scale: u32, floor: &Floored) -> Vec<(u32, u32, Rgb)> {
    let (x0, y0, x1, y1) = zone(pack, scale, floor);
    let all: Vec<_> = (y0..y1)
        .flat_map(|y| (x0..x1).map(move |x| (x, y)))
        .map(|(x, y)| (x, y, pixel(floor, x, y)))
        .collect();
    let [_, _, wide, high] = pack.text_zone.expect("a material pack names its text zone");
    let want = (wide * scale as f32) as usize * (high * scale as f32) as usize;
    assert_eq!(all.len(), want, "{} pixels checked at {scale}x", pack.id);
    all
}

/// The opaque colour of pixel `(x, y)`: un-premultiplied, over black where it is not opaque.
fn pixel(floor: &Floored, x: u32, y: u32) -> Rgb {
    let at = (y * floor.width + x) as usize * 4;
    let p = &floor.bgra[at..at + 4];
    let a = u32::from(p[3]);
    let un = |c: u8| {
        (u32::from(c) * 255)
            .checked_div(a)
            .map_or(0, |v| v.min(255) as u8)
    };
    Rgb([un(p[2]), un(p[1]), un(p[0])])
}

/// The ratio of two colours' luminances.
fn ratio(a: f64, b: f64) -> f64 {
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// The worst (lowest) ratio of `ink` over the zone, with where it happens and the pixel there.
fn worst(pixels: &[(u32, u32, Rgb)], ink: Rgb) -> (f64, u32, u32, Rgb) {
    let li = ink.luminance();
    let mut found = (f64::MAX, 0, 0, Rgb([0, 0, 0]));
    for &(x, y, px) in pixels {
        let r = ratio(li, px.luminance());
        if r < found.0 {
            found = (r, x, y, px);
        }
    }
    found
}

fn describe(name: &str, ink: Rgb, w: (f64, u32, u32, Rgb)) -> String {
    format!(
        "{name} {} worst {:.2} at ({}, {}) on {}",
        ink.to_hex(),
        w.0,
        w.1,
        w.2,
        w.3.to_hex()
    )
}

#[test]
fn floor_png_decodes_at_its_size() {
    let (_, builtin) = &material_packs()[0];
    let one = decode(builtin, "floor-1x");
    assert_eq!((one.width, one.height), (640, 420));
    let two = decode(builtin, "floor-2x");
    assert_eq!((two.width, two.height), (1280, 840));
}

#[test]
fn builtin_text_zones_pass_per_pixel() {
    let mut failures = Vec::new();
    for (pack, builtin) in material_packs() {
        let punch = pack.flow_style == FlowStyle::Punch;
        for (scale, file) in [(1, "floor-1x"), (2, "floor-2x")] {
            let floor = decode(builtin, file);
            let pixels = zone_pixels(&pack, scale, &floor);
            let mut colours = vec![
                ("ink", pack.open.ink.ink, MIN_INK),
                ("muted", pack.open.ink.muted, MIN_MUTED),
                ("faint", pack.open.ink.faint, MIN_FAINT),
            ];
            if !punch {
                colours.push(("flow", pack.flow, MIN_FLOW));
            }
            for (name, ink, min) in colours {
                let w = worst(&pixels, ink);
                println!("{} {file}: {}", pack.id, describe(name, ink, w));
                if w.0 < min {
                    failures.push(format!(
                        "{} {file}: {} (needs {min})",
                        pack.id,
                        describe(name, ink, w)
                    ));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn lum_bounds_are_honest() {
    for (pack, builtin) in material_packs() {
        let Floor::Material {
            lum_min, lum_max, ..
        } = pack.open.floor
        else {
            unreachable!("filtered to material packs");
        };
        for (scale, file) in [(1, "floor-1x"), (2, "floor-2x")] {
            let floor = decode(builtin, file);
            let (mut lo, mut hi) = (f64::MAX, f64::MIN);
            for (_, _, px) in zone_pixels(&pack, scale, &floor) {
                let l = px.luminance();
                lo = lo.min(l);
                hi = hi.max(l);
            }
            println!(
                "{} {file}: lum {lo:.4}..{hi:.4}, manifest {lum_min}..{lum_max}",
                pack.id
            );
            // The manifest rounds to four places.
            let eps = 1e-4;
            assert!(
                lo >= lum_min - eps,
                "{} {file}: darkest {lo} < {lum_min}",
                pack.id
            );
            assert!(
                hi <= lum_max + eps,
                "{} {file}: lightest {hi} > {lum_max}",
                pack.id
            );
        }
    }
}

#[test]
fn the_2x_zone_is_exactly_twice_the_1x_zone() {
    for (pack, builtin) in material_packs() {
        let one = zone(&pack, 1, &decode(builtin, "floor-1x"));
        let two = zone(&pack, 2, &decode(builtin, "floor-2x"));
        assert_eq!(
            two,
            (one.0 * 2, one.1 * 2, one.2 * 2, one.3 * 2),
            "{}",
            pack.id
        );
    }
}

#[test]
#[should_panic(expected = "empty or outside")]
fn an_empty_zone_is_refused() {
    check_rect((10, 10, 10, 50), 640, 420);
}

#[test]
#[should_panic(expected = "empty or outside")]
fn a_zone_past_the_floor_is_refused() {
    check_rect((30, 12, 700, 406), 640, 420);
}
