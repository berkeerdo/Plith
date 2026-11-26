//! Settings sheet renderer: offscreen pixels on WARP, the text measurer and the sheet thread on a
//! hidden window.

#[path = "../examples/render_sheet/facts.rs"]
mod facts;

use plith_core::draw::{ArtId, ArtPixels, DisplayList, Node, Rect, TextStyle, Trim};
use plith_core::lang::Lang;
use plith_core::sheet::{Section, SheetView, sheet_build};
use plith_core::theme::Palette;
use plith_core::theme::colour::Rgb;
use plith_core::theme::registry::{AccentPick, Picks, Registry, ThemeId};
use plith_render::sheet::{SheetConfig, SheetRender, render_sheet_offscreen};
use plith_render::{TextMeasure, TextSystem};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, WINDOW_EX_STYLE, WS_POPUP,
};
use windows::core::w;

fn palette() -> Palette {
    Palette::default()
}

fn idle_list(lang: Lang) -> DisplayList {
    let view = SheetView {
        section: Section::Idle,
        ..SheetView::default()
    };
    sheet_build(&facts::sheet_facts(lang), &view).0
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn byte(c: f32) -> u8 {
    (c * 255.0).round() as u8
}

/// The BGRA pixel at (`x`, `y`) of rows `w` pixels wide.
fn pixel(rows: &[u8], w: usize, x: usize, y: usize) -> [u8; 4] {
    let i = (y * w + x) * 4;
    [rows[i], rows[i + 1], rows[i + 2], rows[i + 3]]
}

#[test]
fn sheet_offscreen_size() {
    let rows = render_sheet_offscreen(&idle_list(Lang::Tr), 2.0, palette(), &[]).unwrap();
    assert_eq!(rows.len(), 1680 * 1280 * 4);
}

#[test]
fn sheet_floor_is_opaque() {
    let rows = render_sheet_offscreen(&idle_list(Lang::Tr), 1.0, palette(), &[]).unwrap();
    let floor = palette().floor;
    let want = [byte(floor[2]), byte(floor[1]), byte(floor[0]), 255];
    for (x, y) in [(0, 0), (839, 0), (0, 639), (839, 639)] {
        assert_eq!(pixel(&rows, 840, x, y), want, "corner ({x}, {y})");
    }
}

#[test]
fn offscreen_draws_sheet_art() {
    let list = DisplayList {
        nodes: vec![Node::Image {
            rect: Rect {
                x: 10.0,
                y: 10.0,
                w: 20.0,
                h: 20.0,
            },
            art: ArtId(7),
        }],
        size: (840.0, 640.0),
    };
    let red = Arc::new(ArtPixels {
        id: ArtId(7),
        width: 2,
        height: 2,
        bgra: Arc::from([0u8, 0, 255, 255].repeat(4)),
    });
    let rows = render_sheet_offscreen(&list, 1.0, palette(), &[red]).unwrap();
    assert_eq!(pixel(&rows, 840, 20, 20), [0, 0, 255, 255]);
    let floor = palette().floor;
    let want = [byte(floor[2]), byte(floor[1]), byte(floor[0]), 255];
    assert_eq!(pixel(&rows, 840, 5, 5), want);
}

#[test]
fn inset_preview_visible() {
    // The first preview (Karot): its time and turn text must put ink inside the inset, 2 DIP in
    // from its edges (clear of the pill's outline), against the pill's raised floor.
    let list = idle_list(Lang::Tr);
    let inset = list
        .nodes
        .iter()
        .find_map(|n| match n {
            Node::Inset {
                x,
                y,
                scale,
                list: inner,
            } => Some((*x, *y, inner.size.0 * scale, inner.size.1 * scale)),
            _ => None,
        })
        .expect("an inset");
    let rows = render_sheet_offscreen(&list, 2.0, palette(), &[]).unwrap();
    let floor = palette().floor_high;
    let base = [byte(floor[2]), byte(floor[1]), byte(floor[0])];
    // Inset coordinates are a few hundred positive DIP.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let px = |dip: f64| (dip * 2.0).round() as usize;
    let (left, top, width, height) = inset;
    let cols = px(left + 2.0)..px(left + width - 2.0);
    let inked = (px(top + 2.0)..px(top + height - 2.0))
        .flat_map(|row| cols.clone().map(move |col| (col, row)))
        .filter(|&(col, row)| {
            let p = pixel(&rows, 1680, col, row);
            (0..3).any(|k| p[k].abs_diff(base[k]) > 64)
        })
        .count();
    assert!(inked > 20, "only {inked} inked pixels in the first preview");
}

#[test]
fn measure_matches_text_system() {
    let mut measure = TextMeasure::new().unwrap();
    let mut text = TextSystem::new().unwrap();
    for s in ["İstanbul", "München", "", "a b "] {
        let got = measure.width(s, TextStyle::Body);
        let want = f64::from(
            text.measure(s, TextStyle::Body, 10_000.0, Trim::End)
                .unwrap(),
        );
        assert!((got - want).abs() < 0.5, "{s:?}: {got} vs {want}");
    }
    assert!(measure.width("İstanbul", TextStyle::Body) > 30.0);
    assert!(measure.width("a ", TextStyle::Body) > measure.width("a", TextStyle::Body));
}

/// A hidden popup the sheet thread can attach a composition target to.
fn hidden_window() -> HWND {
    // SAFETY: a system class, no parent, no menu, no create data; the window is destroyed below.
    unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("STATIC"),
            w!("plith sheet test"),
            WS_POPUP,
            0,
            0,
            840,
            640,
            None,
            None,
            None,
            None,
        )
        .unwrap()
    }
}

/// Waits up to two seconds for `paints()` to reach `n`.
fn wait_paints(render: &SheetRender, n: u64) {
    let start = Instant::now();
    while render.paints() < n && start.elapsed() < Duration::from_secs(2) {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(render.paints(), n);
}

#[test]
fn a_new_palette_repaints_the_last_list_once_and_the_thread_parks_again() {
    let hwnd = hidden_window();
    let config = SheetConfig {
        hwnd: hwnd.0 as isize,
        width_px: 840,
        height_px: 640,
        scale: 1.0,
        palette: palette(),
        art: Vec::new(),
    };
    let render = SheetRender::spawn(config, Box::new(|_| {})).unwrap();
    // No list yet: a palette alone paints nothing.
    render.set_palette(kagit());
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(render.paints(), 0);
    render.publish(Arc::new(idle_list(Lang::Tr)));
    wait_paints(&render, 1);
    render.set_palette(york());
    wait_paints(&render, 2);
    // The same palette again paints nothing.
    render.set_palette(york());
    std::thread::sleep(Duration::from_millis(100));
    let wakes = render.wakes();
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!((render.paints(), render.wakes()), (2, wakes));
    render.stop_and_join();
    // SAFETY: the window was created on this thread and is destroyed once.
    unsafe { DestroyWindow(hwnd).unwrap() };
}

#[test]
fn sheet_thread_paints_on_demand() {
    let hwnd = hidden_window();
    let config = SheetConfig {
        hwnd: hwnd.0 as isize,
        width_px: 840,
        height_px: 640,
        scale: 1.0,
        palette: palette(),
        art: Vec::new(),
    };
    let fatal: Arc<Mutex<Option<String>>> = Arc::default();
    let sink = Arc::clone(&fatal);
    let render =
        SheetRender::spawn(config, Box::new(move |e| *sink.lock().unwrap() = Some(e))).unwrap();
    // Nothing to paint yet: the thread waits.
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(render.paints(), 0);
    let list = Arc::new(idle_list(Lang::Tr));
    render.publish(Arc::clone(&list));
    wait_paints(&render, 1);
    // At rest: no clock, no further wakes or paints.
    let wakes = render.wakes();
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!((render.paints(), render.wakes()), (1, wakes));
    render.resize(1260, 960, 1.5);
    wait_paints(&render, 2);
    render.publish(list);
    wait_paints(&render, 3);
    render.stop_and_join();
    assert_eq!(*fatal.lock().unwrap(), None);
    // SAFETY: the window was created on this thread and is destroyed once.
    unsafe { DestroyWindow(hwnd).unwrap() };
}

/// Gece under a floor and an accent pick, as the settings migration turns a retired theme.
fn migrated(floor: Option<[u8; 3]>, accent: [u8; 3]) -> Palette {
    let accent = AccentPick::Colour(Rgb(accent));
    let picks = Picks {
        accent: Some(&accent),
        floor: floor.map(Rgb),
        windows_accent: None,
    };
    Registry::builtin().resolve(&ThemeId::gece(), picks).open
}

fn kagit() -> Palette {
    migrated(Some([0xF3, 0xF2, 0xED]), [0x3F, 0x6E, 0x00])
}

fn york() -> Palette {
    migrated(Some([0x1C, 0x24, 0x20]), [0xFF, 0xE9, 0xA8])
}
