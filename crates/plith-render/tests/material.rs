#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
//! Offscreen tests of a material theme on WARP: the Halka fabric floor, the black pill, the
//! content palettes, a switch back to a plain theme and the plain path against a frame painted
//! before materials existed.

use plith_core::ambient::LiveValues;
use plith_core::draw::{ArtId, ArtPixels, DisplayList, Ink, Node, PageContent, Rect, StationMark};
use plith_core::geometry::IslandShape;
use plith_core::lang::Lang;
use plith_core::material::MaterialLayers;
use plith_core::material::art::{MaterialArt, ThemePaint};
use plith_core::page::{PageLayer, PageLayers};
use plith_core::pages::PageId;
use plith_core::pages::audio::{PickMode, audio_page, audio_view};
use plith_core::pages::fixtures;
use plith_core::shapes::{REST, REST_HIDDEN};
use plith_core::theme::Palette;
use plith_core::theme::builtin::{BUILTIN, BuiltinPack};
use plith_core::theme::colour::Rgb;
use plith_core::theme::pack::{Floor, parse_pack};
use plith_core::theme::registry::{Picks, Registry, ThemeId};
use plith_render::TextSystem;
use plith_render::offscreen::Offscreen;
use plith_render::paint::PaintFrame;
use std::sync::Arc;
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};

fn com() {
    // SAFETY: initializes COM for this test thread; a repeat call returns S_FALSE and is fine.
    let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
}

fn halka_pack() -> &'static BuiltinPack {
    BUILTIN
        .iter()
        .find(|b| parse_pack(b.manifest).is_ok_and(|p| p.id == "halka"))
        .expect("halka is built in")
}

fn decode(pack: &BuiltinPack, name: &str) -> Arc<ArtPixels> {
    com();
    let bytes = pack
        .files
        .iter()
        .find(|(file, _)| *file == name)
        .unwrap_or_else(|| panic!("no file {name}"))
        .1;
    let art = plith_media::art::decode_image(bytes, u32::MAX)
        .unwrap_or_else(|| panic!("{name} does not decode"));
    Arc::new(ArtPixels {
        id: ArtId(art.id),
        width: art.width,
        height: art.height,
        bgra: art.bgra,
    })
}

/// The Halka paint at the 1x files, as the app sends it.
fn halka() -> ThemePaint {
    halka_files("1x")
}

/// The Halka paint at the `files` scale class (`1x` or `2x`).
fn halka_files(files: &str) -> ThemePaint {
    let pack = halka_pack();
    let get = |base: &str| decode(pack, &format!("{base}-{files}"));
    let art = MaterialArt {
        fabric: get("floor"),
        rings: [get("ring-1"), get("ring-2"), get("ring-3")],
        disc: get("disc"),
        pill: get("pill"),
    };
    let id = ThemeId::parse("halka").expect("id");
    let resolved = Registry::builtin().resolve(
        &id,
        Picks {
            accent: None,
            floor: None,
            windows_accent: None,
        },
    );
    assert!(resolved.material, "halka draws a material");
    ThemePaint::new(id, resolved.open, resolved.collapsed, Some(Arc::new(art)))
}

/// The pack's measured mean fabric colour.
fn fabric_mean() -> Rgb {
    let pack = parse_pack(halka_pack().manifest).expect("parses");
    match pack.open.floor {
        Floor::Material { mean, .. } => mean,
        Floor::Colour { .. } => panic!("halka has a material floor"),
    }
}

fn offscreen(w: u32, h: u32) -> Offscreen {
    let text = TextSystem::new().expect("text system");
    Offscreen::new(w, h, 1.0, Palette::default(), text).expect("offscreen")
}

fn px_size(shape: IslandShape) -> (u32, u32) {
    (shape.width.ceil() as u32, shape.height.ceil() as u32)
}

fn pixel(buf: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * width + x) * 4) as usize;
    [buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]
}

/// The mean of the `(2r + 1)` square around `(x, y)` as BGRA: the weave is noise, so a floor
/// check compares a small neighbourhood, not one thread.
fn box_mean(buf: &[u8], width: u32, (cx, cy): (u32, u32), reach: u32) -> [u8; 4] {
    let mut sum = [0u32; 4];
    let mut count = 0;
    for row in cy - reach..=cy + reach {
        for col in cx - reach..=cx + reach {
            let px = pixel(buf, width, col, row);
            for (total, channel) in sum.iter_mut().zip(px) {
                *total += u32::from(channel);
            }
            count += 1;
        }
    }
    sum.map(|total| ((total + count / 2) / count) as u8)
}

fn rgb_bgra(c: Rgb) -> [u8; 4] {
    let [r, g, b] = c.0;
    [b, g, r, 255]
}

fn bgra(c: [f32; 4]) -> [u8; 4] {
    let b = |v: f32| (v * 255.0).round() as u8;
    [b(c[2]), b(c[1]), b(c[0]), 255]
}

fn near(got: [u8; 4], want: [u8; 4], tol: u8, what: &str) {
    let ok = got.iter().zip(want).all(|(g, w)| g.abs_diff(w) <= tol);
    assert!(ok, "{what}: got {got:?}, want {want:?} within {tol}");
}

/// WCAG luminance of an opaque BGRA pixel.
fn luminance(p: [u8; 4]) -> f64 {
    Rgb([p[2], p[1], p[0]]).luminance()
}

fn frame(
    shape: IslandShape,
    content: PageContent,
    opacity: f32,
    layers: MaterialLayers,
) -> PaintFrame {
    let pages: Vec<PageLayer> = if opacity > 0.0 {
        vec![PageLayer {
            page: 0,
            offset: 0.0,
            opacity: 1.0,
        }]
    } else {
        Vec::new()
    };
    PaintFrame {
        shape,
        origin: (0.0, 0.0),
        pages: PageLayers::of(&pages),
        content_opacity: opacity,
        content: Arc::new(content),
        content_gen: 1,
        live: LiveValues::default(),
        material: layers,
    }
}

/// The open audio page at rest, with its shape.
fn audio_frame() -> PaintFrame {
    let view = audio_view(
        &fixtures::lit(&fixtures::banana()),
        PickMode::None,
        Lang::Tr,
    );
    let build = audio_page(&view, fixtures::CLOCK, Lang::Tr);
    let mut content = PageContent::empty();
    content.pages[PageId::Audio.index()] = Arc::new(build.list.clone());
    let mut f = frame(build.shape, content, 1.0, MaterialLayers::OPEN);
    f.pages = PageLayers::of(&[PageLayer {
        page: PageId::Audio.index(),
        offset: 0.0,
        opacity: 1.0,
    }]);
    f
}

fn fill(x: f64, y: f64, w: f64, h: f64, ink: Ink) -> Node {
    Node::Fill {
        rect: Rect { x, y, w, h },
        radius: 0.0,
        ink,
    }
}

#[test]
fn halka_open_floor_is_fabric() {
    let f = audio_frame();
    let (w, h) = px_size(f.shape);
    let mut off = offscreen(w, h);
    off.set_theme(halka());
    let buf = off.render(&f).unwrap();
    assert_eq!(pixel(&buf, w, 20, 200)[3], 255, "opaque floor");
    near(
        box_mean(&buf, w, (20, 200), 3),
        rgb_bgra(fabric_mean()),
        12,
        "fabric at (20, 200)",
    );
    // A raster, not the flat mean colour: the weave varies thread to thread.
    let first = pixel(&buf, w, 17, 197);
    let woven = (197..=203).any(|y| (17..=23).any(|x| pixel(&buf, w, x, y) != first));
    assert!(
        woven,
        "the floor at (20, 200) is a flat colour, not the fabric"
    );
}

#[test]
fn halka_collapsed_is_black() {
    let (w, h) = px_size(REST);
    let mut off = offscreen(w, h);
    off.set_theme(halka());
    let buf = off
        .render(&frame(
            REST,
            PageContent::empty(),
            0.0,
            MaterialLayers::COLLAPSED,
        ))
        .unwrap();
    let centre = pixel(&buf, w, 100, 16);
    assert_eq!(centre[3], 255, "opaque pill (the base under its 253 body)");
    assert!(luminance(centre) < 0.1, "black pill, got {centre:?}");
}

#[test]
fn halka_hidden_sliver_is_black_not_fabric() {
    let (w, h) = px_size(REST_HIDDEN);
    let mut off = offscreen(w, h);
    off.set_theme(halka());
    let buf = off
        .render(&frame(
            REST_HIDDEN,
            PageContent::empty(),
            0.0,
            MaterialLayers::COLLAPSED,
        ))
        .unwrap();
    for x in [10, 100, 190] {
        let p = pixel(&buf, w, x, 1);
        assert_eq!(p[3], 255, "opaque sliver at {x}");
        assert!(luminance(p) < 0.1, "black sliver at {x}, got {p:?}");
    }
}

/// The Hidden sliver at `scale` with the files the app uses there: no light pixel anywhere in
/// it, the two ends included (the pill caps' rim highlight must not show in 3 DIP).
fn assert_sliver_ends_black(scale: f64, files: &str) {
    let w = (REST_HIDDEN.width * scale).ceil() as u32;
    let h = (REST_HIDDEN.height * scale).ceil() as u32;
    let text = TextSystem::new().expect("text system");
    let mut off = Offscreen::new(w, h, scale, Palette::default(), text).expect("offscreen");
    off.set_theme(halka_files(files));
    let buf = off
        .render(&frame(
            REST_HIDDEN,
            PageContent::empty(),
            0.0,
            MaterialLayers::COLLAPSED,
        ))
        .unwrap();
    // The ends are the caps' 32 DIP; the border's outer device pixels are left out.
    let edge = scale.ceil() as u32;
    let cap = (32.0 * scale) as u32;
    for y in 0..h - edge {
        for x in (edge..cap).chain(w - cap..w - edge) {
            let p = pixel(&buf, w, x, y);
            let max = p[0].max(p[1]).max(p[2]);
            assert!(max <= 0x18, "light pixel at ({x}, {y}) at {scale}x: {p:?}");
        }
    }
}

#[test]
fn halka_hidden_sliver_ends_are_black() {
    assert_sliver_ends_black(1.0, "1x");
    assert_sliver_ends_black(2.0, "2x");
}

#[test]
fn the_pill_keeps_its_height_while_it_fades() {
    // A taller island mid-fade: the pill stays 32 DIP at the top, the base shows below it.
    let shape = IslandShape {
        width: 300.0,
        height: 120.0,
        radius: 24.0,
    };
    let (w, h) = px_size(shape);
    let mut off = offscreen(w, h);
    let paint = halka();
    let base = bgra(paint.collapsed.floor);
    off.set_theme(paint);
    let layers = MaterialLayers {
        pill: 1.0,
        ..MaterialLayers::COLLAPSED
    };
    let buf = off
        .render(&frame(shape, PageContent::empty(), 0.0, layers))
        .unwrap();
    for at in [(150, 60), (150, 100), (14, 90), (286, 90)] {
        near(
            pixel(&buf, w, at.0, at.1),
            base,
            2,
            &format!("base at {at:?}"),
        );
    }
}

#[test]
fn a_ring_shows_only_inside_the_reveal() {
    // Progress about 0.27 of a strip frame: ring 1 pressed 0.8, the circle at 0.49 of the far
    // corner (radius about 93 DIP from (181.6, 33)).
    let shape = IslandShape {
        width: 363.2,
        height: 85.9,
        radius: 24.0,
    };
    let (w, h) = px_size(shape);
    let mut off = offscreen(w, h);
    off.set_theme(halka());
    let layers = MaterialLayers {
        reveal: 0.49,
        ripples: [0.8, 0.0, 0.0],
        disc: 0.0,
        disc_lift: 0.0,
        pill: 0.0,
    };
    let buf = off
        .render(&frame(shape, PageContent::empty(), 0.0, layers))
        .unwrap();
    // Inside ring 1's raster (x 72.6 to 289.6, y 24.8 to 131.8), outside the circle.
    for at in [(78, 50), (285, 50), (80, 70)] {
        let p = pixel(&buf, w, at.0, at.1);
        assert!(
            luminance(p) < 0.05,
            "ring outside the reveal at {at:?}: {p:?}"
        );
    }
}

#[test]
fn fabric_brush_extends_by_mirror() {
    let shape = IslandShape {
        width: 651.0,
        height: 441.0,
        radius: 24.0,
    };
    let (w, h) = px_size(shape);
    let mut off = offscreen(w, h);
    off.set_theme(halka());
    let buf = off
        .render(&frame(
            shape,
            PageContent::empty(),
            1.0,
            MaterialLayers::OPEN,
        ))
        .unwrap();
    let mean = rgb_bgra(fabric_mean());
    // Beyond the 640 x 420 raster on both axes, inside the rounded corner.
    for at in [(646, 425), (647, 4), (14, 432)] {
        assert_eq!(pixel(&buf, w, at.0, at.1)[3], 255, "opaque at {at:?}");
        near(
            box_mean(&buf, w, at, 2),
            mean,
            12,
            "fabric beyond the raster",
        );
    }
    // A clamped edge would repeat one column: past the right edge the columns must differ.
    let differs = (200..240).any(|y| pixel(&buf, w, 647, y) != pixel(&buf, w, 650, y));
    assert!(differs, "columns past the raster edge are a clamped stripe");
}

#[test]
fn theme_switch_mid_open_replaces_floor() {
    let f = audio_frame();
    let (w, h) = px_size(f.shape);
    let mut off = offscreen(w, h);
    off.set_theme(halka());
    let _ = off.render(&f).unwrap();
    let gece = Palette::default();
    off.set_theme(ThemePaint::plain(gece));
    let buf = off.render(&f).unwrap();
    let at = 200.0 / f.shape.height;
    let mix = |top: f32, low: f32| {
        ((f64::from(top) + (f64::from(low) - f64::from(top)) * at) * 255.0).round() as u8
    };
    let (top, low) = (gece.floor, gece.floor_high);
    let want = [
        mix(top[2], low[2]),
        mix(top[1], low[1]),
        mix(top[0], low[0]),
        255,
    ];
    near(
        pixel(&buf, w, 20, 200),
        want,
        6,
        "Gece floor after the switch",
    );
}

#[test]
fn halka_content_keeps_its_own_palette() {
    let theme = halka();
    let mut content = PageContent::empty();
    content.pages[0] = Arc::new(DisplayList {
        nodes: vec![fill(40.0, 60.0, 30.0, 20.0, Ink::Ink)],
        size: (REST.width, 120.0),
    });
    content.pill = Arc::new(DisplayList {
        nodes: vec![fill(40.0, 8.0, 30.0, 16.0, Ink::Ink)],
        size: (REST.width, REST.height),
    });
    let content = Arc::new(content);
    let open_shape = IslandShape {
        width: REST.width,
        height: 120.0,
        radius: 24.0,
    };
    let (w, h) = px_size(open_shape);
    let mut off = offscreen(w, h);
    off.set_theme(theme.clone());
    let mut open = frame(open_shape, PageContent::empty(), 1.0, MaterialLayers::OPEN);
    open.content = Arc::clone(&content);
    let buf = off.render(&open).unwrap();
    near(
        pixel(&buf, w, 55, 70),
        bgra(theme.open.ink),
        2,
        "page in the open ink",
    );
    let mut shut = frame(REST, PageContent::empty(), 0.0, MaterialLayers::COLLAPSED);
    shut.content = content;
    let buf = off.render(&shut).unwrap();
    near(
        pixel(&buf, w, 55, 16),
        bgra(theme.collapsed.ink),
        2,
        "pill in the collapsed ink",
    );
}

#[test]
fn material_bitmaps_are_built_once_per_theme() {
    let f = audio_frame();
    let (w, h) = px_size(f.shape);
    let mut off = offscreen(w, h);
    let theme = halka();
    off.set_theme(theme.clone());
    for _ in 0..3 {
        let _ = off.render(&f).unwrap();
    }
    assert_eq!(off.material_builds(), 1, "one build for three frames");
    off.set_theme(theme);
    let _ = off.render(&f).unwrap();
    assert_eq!(off.material_builds(), 1, "the same art is not built again");
    off.set_theme(ThemePaint::plain(Palette::default()));
    let _ = off.render(&f).unwrap();
    off.set_theme(halka());
    let _ = off.render(&f).unwrap();
    assert_eq!(off.material_builds(), 2, "new art after a plain theme");
}

/// The frame of `fixtures/gece-base.png`: written at commit 985270e, before the painter knew
/// materials, with a scratch copy of this function.
fn base_frame() -> PaintFrame {
    let shape = IslandShape {
        width: 300.0,
        height: 140.0,
        radius: 24.0,
    };
    let mut content = PageContent::empty();
    content.pages[0] = Arc::new(DisplayList {
        nodes: vec![
            Node::Fill {
                rect: Rect {
                    x: 20.0,
                    y: 20.0,
                    w: 80.0,
                    h: 30.0,
                },
                radius: 6.0,
                ink: Ink::Ink,
            },
            Node::Stroke {
                points: vec![(20.0, 80.0), (200.0, 80.0)],
                width: 2.0,
                ink: Ink::Flow,
                dashed: false,
            },
            Node::Station {
                x: 240.0,
                y: 60.0,
                mark: StationMark::Filled,
                ink: Ink::Ink,
            },
        ],
        size: (300.0, 140.0),
    });
    content.pill = Arc::new(DisplayList {
        nodes: vec![Node::Fill {
            rect: Rect {
                x: 10.0,
                y: 8.0,
                w: 40.0,
                h: 16.0,
            },
            radius: 4.0,
            ink: Ink::Flow,
        }],
        size: (200.0, 32.0),
    });
    let mut f = frame(shape, content, 0.6, MaterialLayers::OPEN);
    f.origin = (6.0, 6.0);
    f
}

#[test]
fn plain_theme_paints_as_before() {
    com();
    let png = include_bytes!("fixtures/gece-base.png");
    let base = plith_media::art::decode_image(png, u32::MAX).expect("fixture decodes");
    assert_eq!((base.width, base.height), (312, 152));
    let worst = |buf: &[u8]| {
        assert_eq!(buf.len(), base.bgra.len());
        buf.iter()
            .zip(base.bgra.iter())
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap_or(0)
    };
    // The fixture is a straight-alpha PNG of the premultiplied frame: 2 levels of rounding.
    let mut off = offscreen(312, 152);
    let fresh = worst(&off.render(&base_frame()).unwrap());
    assert!(
        fresh <= 2,
        "plain frame drifted by {fresh} levels from the base"
    );
    // Through a material theme and back: the plain path must not keep anything of it.
    off.set_theme(halka());
    let _ = off.render(&base_frame()).unwrap();
    off.set_theme(ThemePaint::plain(Palette::default()));
    let back = worst(&off.render(&base_frame()).unwrap());
    assert!(
        back <= 2,
        "plain frame after halka drifted by {back} levels"
    );
}

fn open_shape() -> IslandShape {
    IslandShape {
        width: 620.0,
        height: 300.0,
        radius: 24.0,
    }
}

#[test]
fn the_disc_and_rings_draw_while_a_motion_runs() {
    let shape = open_shape();
    let (w, h) = px_size(shape);
    let mut off = offscreen(w, h);
    off.set_theme(halka());
    let rest = off
        .render(&frame(
            shape,
            PageContent::empty(),
            1.0,
            MaterialLayers::OPEN,
        ))
        .unwrap();
    let pressed = MaterialLayers {
        reveal: 1.0,
        ripples: [1.0; 3],
        disc: 1.0,
        disc_lift: 0.0,
        pill: 0.0,
    };
    let buf = off
        .render(&frame(shape, PageContent::empty(), 1.0, pressed))
        .unwrap();
    let disc = pixel(&buf, w, 310, 20);
    assert!(
        luminance(disc) < 0.1,
        "the source disc is black, got {disc:?}"
    );
    // Ring 3 spans y 95 to 225 below the source: its pixels differ from the flat fabric.
    let changed = (100..220)
        .filter(|&y| pixel(&buf, w, 310, y) != pixel(&rest, w, 310, y))
        .count();
    assert!(
        changed > 20,
        "the rings press into the fabric ({changed} rows changed)"
    );
}

#[test]
fn a_partial_reveal_is_a_circle_from_the_source() {
    let shape = open_shape();
    let (w, h) = px_size(shape);
    let mut off = offscreen(w, h);
    off.set_theme(halka());
    let layers = MaterialLayers {
        reveal: 0.3,
        ..MaterialLayers::OPEN
    };
    let buf = off
        .render(&frame(shape, PageContent::empty(), 1.0, layers))
        .unwrap();
    // Far corner distance is hypot(310, 267) = 409; 0.3 of it is 123 DIP around (310, 33).
    let inside = pixel(&buf, w, 310, 140);
    assert!(
        luminance(inside) > 0.5,
        "fabric inside the circle, got {inside:?}"
    );
    let outside = pixel(&buf, w, 20, 280);
    assert_eq!(outside[3], 255, "the base fills outside the circle");
    assert!(
        luminance(outside) < 0.1,
        "black outside the circle, got {outside:?}"
    );
}

#[test]
fn halka_mid_expand_is_opaque() {
    let shape = IslandShape {
        width: 620.0,
        height: 400.0,
        radius: 24.0,
    };
    let (w, h) = px_size(shape);
    let mut off = offscreen(w, h);
    off.set_theme(halka());
    // About 0.40 of the morph: the pill gone, the reveal short of the far corner.
    let layers = MaterialLayers {
        reveal: 0.4 / 0.55,
        ripples: [1.0, 0.5, 0.0],
        disc: 1.0,
        disc_lift: 5.0,
        pill: 0.0,
    };
    let buf = off
        .render(&frame(shape, PageContent::empty(), 0.0, layers))
        .unwrap();
    // Far corner hypot(310, 367) = 480; the circle reaches 349 DIP from (310, 33).
    for at in [(20, 380), (600, 380), (310, 395)] {
        let p = pixel(&buf, w, at.0, at.1);
        assert_eq!(p[3], 255, "opaque outside the reveal at {at:?}");
        assert!(luminance(p) < 0.1, "black base at {at:?}, got {p:?}");
    }
}
