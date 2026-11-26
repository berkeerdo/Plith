//! Renders every page state offscreen on WARP at scale 2 and writes PNGs for review against the
//! R3 mock: each island sits flush under a black screen edge on a desk cell, like the mock's
//! cells. Output goes to a new folder `%TEMP%\plith-renders-<unix seconds>-<pid>`; no window
//! opens.
//!
//! Run: `cargo run --release -p plith-render --example render_pages`. With a folder argument
//! (`-- <new folder>`) it renders only the idle look states of the R7 mock (the welcome view and
//! the six pill looks, from a fixed day and time) into that folder, which must not exist yet.
//! `--shelf <new folder>` renders the shelf page with 0 to 8 wagons and the hot slot.
//! `--quick <new folder>` renders the quick page and every tab row at 1x and 2x in Turkish and
//! German. `--lang <code>` picks the language of our own copy (default `tr`; an unknown code is an
//! error) and `--out <new folder>` names the folder of the page states.

use plith_core::draw::{DisplayList, PageContent};
use plith_core::geometry::IslandShape;
use plith_core::lang::Lang;
use plith_core::material::MaterialLayers;
use plith_core::page::{PageLayer, PageLayers};
use plith_core::pages::audio::{PickMode, audio_page, audio_view};
use plith_core::pages::fixtures;
use plith_core::pages::media::media_page;
use plith_core::pages::pill::pill;
use plith_core::pages::shelf::shelf_page;
use plith_core::pages::system::system_page;
use plith_core::pages::{PageBuild, PageId};
use plith_core::shapes::REST;
use plith_core::theme::colour::Rgb;
use plith_core::theme::registry::{AccentPick, Picks, Registry, ThemeId};
use plith_core::theme::{Palette, hex};
use plith_render::paint::PaintFrame;
use plith_render::{TextSystem, render_offscreen};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
#[path = "render_pages/idle_states.rs"]
mod idle_states;
#[path = "render_pages/quick_states.rs"]
mod quick_states;
#[path = "render_pages/shelf_states.rs"]
mod shelf_states;

use windows::Win32::Foundation::GENERIC_WRITE;
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_ContainerFormatPng, GUID_WICPixelFormat32bppBGRA,
    IWICBitmapFrameEncode, IWICImagingFactory, WICBitmapEncoderNoCache,
};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
};
use windows::core::{HSTRING, Result};

thread_local! {
    static SCALE_CELL: std::cell::Cell<f64> = const { std::cell::Cell::new(2.0) };
}

/// The scale of this run (2 unless the quick check sets another).
fn scale() -> f64 {
    SCALE_CELL.with(std::cell::Cell::get)
}
/// Cell width in DIP (the mock's grid column).
const CELL_W: f64 = 680.0;
/// The black screen edge above the island, in DIP (the mock's `.edge`).
const EDGE: f64 = 2.0;
/// Desk below the island, in DIP.
const BOTTOM: f64 = 30.0;
/// Gap between pills in the pill cell, in DIP (the mock's row gap).
const PILL_GAP: f64 = 40.0;

/// An opaque BGRA canvas in pixels.
struct Canvas {
    w: usize,
    h: usize,
    px: Vec<u8>,
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn px(dip: f64) -> usize {
    // DIP sizes are small positive values; ceil keeps the edge pixels.
    (dip * scale()).ceil() as usize
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn byte(c: f32) -> u8 {
    // Palette channels are 0..=1.
    (c * 255.0).round() as u8
}

impl Canvas {
    /// A desk cell of `w` by `h` DIP with the black screen edge on top.
    fn desk(w: f64, h: f64, desk: [f32; 4]) -> Self {
        let (w, h) = (px(w), px(h));
        let mut px_buf = Vec::with_capacity(w * h * 4);
        let edge = px(EDGE);
        for y in 0..h {
            let c = if y < edge {
                [0, 0, 0, 255]
            } else {
                [byte(desk[2]), byte(desk[1]), byte(desk[0]), 255]
            };
            for _ in 0..w {
                px_buf.extend_from_slice(&c);
            }
        }
        Self { w, h, px: px_buf }
    }

    /// Composites premultiplied BGRA rows (`sw` by `sh`) at pixel (`x`, `y`).
    fn over(&mut self, src: &[u8], sw: usize, sh: usize, x: usize, y: usize) {
        for row in 0..sh.min(self.h.saturating_sub(y)) {
            for col in 0..sw.min(self.w.saturating_sub(x)) {
                let s = (row * sw + col) * 4;
                let d = ((y + row) * self.w + x + col) * 4;
                let keep = 255 - u16::from(src[s + 3]);
                for k in 0..3 {
                    let under = u16::from(self.px[d + k]) * keep / 255;
                    self.px[d + k] = u8::try_from(u16::from(src[s + k]) + under).unwrap_or(255);
                }
            }
        }
    }
}

/// What one island shows.
enum Content {
    Page(PageId, PageBuild),
    Pill(DisplayList, IslandShape),
}

/// Renders one island at the origin of a target its own size; returns rows and pixel size.
fn island(content: &Content, palette: Palette) -> Result<(Vec<u8>, usize, usize)> {
    let mut page_content = PageContent::empty();
    page_content.art = vec![Arc::new(fixtures::art())];
    let (shape, layers, opacity): (IslandShape, Vec<PageLayer>, f32) = match content {
        Content::Page(id, build) => {
            page_content.pages[id.index()] = Arc::new(build.list.clone());
            let layer = PageLayer {
                page: id.index(),
                offset: 0.0,
                opacity: 1.0,
            };
            (build.shape, vec![layer], 1.0)
        }
        Content::Pill(list, shape) => {
            page_content.pill = Arc::new(list.clone());
            (*shape, Vec::new(), 0.0)
        }
    };
    let frame = PaintFrame {
        shape,
        origin: (0.0, 0.0),
        pages: PageLayers::of(&layers),
        content_opacity: opacity,
        content: Arc::new(page_content),
        content_gen: 1,
        live: fixtures::live_values(),
        material: MaterialLayers::OPEN,
    };
    let (w, h) = (px(shape.width), px(shape.height));
    let text = TextSystem::new()?;
    #[allow(clippy::cast_possible_truncation)]
    // Island sizes are a few hundred DIP.
    let rows = render_offscreen(w as u32, h as u32, scale(), &frame, palette, text)?;
    Ok((rows, w, h))
}

/// One look of the mock: a palette on the colour of the desk it stands on.
#[derive(Clone, Copy)]
struct Look {
    palette: Palette,
    desk: [f32; 4],
}

impl Look {
    /// Gece, the default pack, on its desk.
    fn gece() -> Self {
        Self {
            palette: Palette::default(),
            desk: hex(0x1B, 0x1D, 0x21),
        }
    }

    /// Gece under a floor and an accent pick, the way a retired theme migrates.
    fn picked(floor: Option<[u8; 3]>, accent: [u8; 3], desk: [f32; 4]) -> Self {
        let accent = AccentPick::Colour(Rgb(accent));
        let picks = Picks {
            accent: Some(&accent),
            floor: floor.map(Rgb),
            windows_accent: None,
        };
        let resolved = Registry::builtin().resolve(&ThemeId::gece(), picks);
        Self {
            palette: resolved.open,
            desk,
        }
    }
}

fn shape_of(content: &Content) -> IslandShape {
    match content {
        Content::Page(_, build) => build.shape,
        Content::Pill(_, shape) => *shape,
    }
}

/// A cell with the islands side by side, centred, flush under the edge.
fn cell(contents: &[Content], look: Look) -> Result<Canvas> {
    let shapes: Vec<IslandShape> = contents.iter().map(shape_of).collect();
    let row_w: f64 = shapes.iter().map(|s| s.width).sum::<f64>()
        + PILL_GAP * f64::from(u32::try_from(shapes.len()).unwrap_or(1).saturating_sub(1));
    let tallest = shapes.iter().map(|s| s.height).fold(0.0, f64::max);
    let mut canvas = Canvas::desk(CELL_W, EDGE + tallest + BOTTOM, look.desk);
    let mut x = (CELL_W - row_w) / 2.0;
    for (content, shape) in contents.iter().zip(&shapes) {
        let (rows, w, h) = island(content, look.palette)?;
        canvas.over(&rows, w, h, px(x), px(EDGE));
        x += shape.width + PILL_GAP;
    }
    Ok(canvas)
}

/// Writes `canvas` as a PNG with WIC.
fn write_png(path: &Path, canvas: &Canvas) -> Result<()> {
    let (w, h) = (
        u32::try_from(canvas.w).unwrap_or(0),
        u32::try_from(canvas.h).unwrap_or(0),
    );
    // SAFETY: COM is initialized on this thread before the factory is created; every pointer
    // passed points to a live local, and the pixel slice holds `h` rows of `w * 4` bytes.
    unsafe {
        let factory: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
        let stream = factory.CreateStream()?;
        stream.InitializeFromFilename(&HSTRING::from(path.as_os_str()), GENERIC_WRITE.0)?;
        let encoder = factory.CreateEncoder(&GUID_ContainerFormatPng, std::ptr::null())?;
        encoder.Initialize(&stream, WICBitmapEncoderNoCache)?;
        let mut frame: Option<IWICBitmapFrameEncode> = None;
        encoder.CreateNewFrame(&raw mut frame, std::ptr::null_mut())?;
        let frame = frame.ok_or_else(windows::core::Error::empty)?;
        frame.Initialize(None)?;
        frame.SetSize(w, h)?;
        let mut format = GUID_WICPixelFormat32bppBGRA;
        frame.SetPixelFormat(&raw mut format)?;
        frame.WritePixels(h, w * 4, &canvas.px)?;
        frame.Commit()?;
        encoder.Commit()?;
    }
    Ok(())
}

/// Every Gece state, by file name.
fn gece_states(lang: Lang) -> Vec<(&'static str, Vec<Content>)> {
    let clock = fixtures::CLOCK;
    let audio = |facts, mode| {
        let view = audio_view(&fixtures::lit(&facts), mode, lang);
        vec![Content::Page(PageId::Audio, audio_page(&view, clock, lang))]
    };
    let media = |facts| {
        vec![Content::Page(
            PageId::Media,
            media_page(&facts, clock, lang),
        )]
    };
    let system = |facts| {
        vec![Content::Page(
            PageId::System,
            system_page(&facts, clock, lang),
        )]
    };
    vec![
        ("01-audio-banana", audio(fixtures::banana(), PickMode::None)),
        (
            "02-audio-pick",
            audio(fixtures::banana(), PickMode::Picked(0)),
        ),
        ("03-audio-potato", audio(fixtures::potato(), PickMode::None)),
        (
            "04-audio-no-voicemeeter",
            audio(fixtures::no_voicemeeter(), PickMode::None),
        ),
        (
            "04b-audio-no-voicemeeter-muted",
            audio(fixtures::no_voicemeeter_muted(), PickMode::None),
        ),
        (
            "04c-audio-no-voicemeeter-no-outputs",
            audio(fixtures::no_voicemeeter_no_endpoints(), PickMode::None),
        ),
        (
            "04d-audio-no-voicemeeter-many",
            audio(fixtures::no_voicemeeter_many(), PickMode::None),
        ),
        ("05-media-playing", media(fixtures::media_playing())),
        ("06-media-empty", media(fixtures::media_empty())),
        (
            "07-shelf-empty",
            vec![Content::Page(PageId::Shelf, shelf_page(clock, lang))],
        ),
        ("08-system", system(fixtures::system())),
        ("09-system-mic-muted", system(fixtures::system_mic_muted())),
        ("10-system-no-ddc", system(fixtures::system_no_ddc())),
        (
            "11-pills",
            fixtures::pills()
                .iter()
                .map(|p| Content::Pill(pill(p, lang), REST))
                .collect(),
        ),
    ]
}

/// A new folder: `dir` when named (its parents are made, the folder itself must not exist),
/// else a uniquely named one under `%TEMP%`.
fn out_dir(dir: Option<&Path>) -> std::io::Result<PathBuf> {
    let dir = dir.map_or_else(
        || {
            let secs = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_secs());
            std::env::temp_dir().join(format!("plith-renders-{secs}-{}", std::process::id()))
        },
        Path::to_path_buf,
    );
    if let Some(parent) = dir.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::create_dir(&dir)?;
    Ok(dir)
}

/// The command line: `--lang <code>`, `--out <dir>` and an optional folder for the idle mode.
struct Args {
    lang: Lang,
    out: Option<PathBuf>,
    idle: Option<PathBuf>,
    quick: Option<PathBuf>,
    shelf: Option<PathBuf>,
}

fn parse_args() -> std::result::Result<Args, String> {
    let mut args = Args {
        lang: Lang::Tr,
        out: None,
        idle: None,
        quick: None,
        shelf: None,
    };
    let mut it = std::env::args_os().skip(1);
    while let Some(arg) = it.next() {
        match arg.to_str() {
            Some("--lang") => {
                let code = it.next().ok_or("--lang needs a code")?;
                let code = code.to_string_lossy();
                args.lang = Lang::ALL
                    .into_iter()
                    .find(|l| l.code() == code)
                    .ok_or_else(|| format!("unknown language code `{code}`"))?;
            }
            Some("--quick") => args.quick = Some(it.next().ok_or("--quick needs a folder")?.into()),
            Some("--shelf") => args.shelf = Some(it.next().ok_or("--shelf needs a folder")?.into()),
            Some("--out") => args.out = Some(it.next().ok_or("--out needs a folder")?.into()),
            Some(flag) if flag.starts_with("--") => return Err(format!("unknown option {flag}")),
            _ if args.idle.is_none() => args.idle = Some(arg.into()),
            _ => return Err("only one idle folder may be given".into()),
        }
    }
    Ok(args)
}

/// Renders the idle look states in `lang` into the new folder `dir`.
fn render_idle(dir: &Path, lang: Lang) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let dir = out_dir(Some(dir))?;
    for (name, contents) in idle_states::states(lang) {
        write_png(
            &dir.join(format!("{name}.png")),
            &cell(&contents, Look::gece())?,
        )?;
    }
    println!("{}", dir.display());
    Ok(())
}

/// Renders the quick page and every tab row at 1x and 2x, Turkish and German, into the new
/// folder `dir`.
fn render_quick(dir: &Path) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let dir = out_dir(Some(dir))?;
    for (code, lang) in [("tr", Lang::Tr), ("de", Lang::De)] {
        for factor in [1.0, 2.0] {
            SCALE_CELL.with(|s| s.set(factor));
            for (name, contents) in quick_states::states(lang) {
                let file = format!("{name}-{code}-{factor}x.png");
                write_png(&dir.join(file), &cell(&contents, Look::gece())?)?;
            }
        }
    }
    println!("{}", dir.display());
    Ok(())
}

/// Renders the shelf page states in `lang` into the new folder `dir`.
fn render_shelf(dir: &Path, lang: Lang) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let dir = out_dir(Some(dir))?;
    for (name, contents) in shelf_states::states(lang) {
        write_png(
            &dir.join(format!("{name}.png")),
            &cell(&contents, Look::gece())?,
        )?;
    }
    println!("{}", dir.display());
    Ok(())
}

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // SAFETY: called once on the main thread before any COM use.
    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok()? };
    let args = parse_args()?;
    let lang = args.lang;
    if let Some(dir) = &args.shelf {
        return render_shelf(dir, lang);
    }
    if let Some(dir) = &args.quick {
        return render_quick(dir);
    }
    if let Some(dir) = &args.idle {
        return render_idle(dir, lang);
    }
    let dir = out_dir(args.out.as_deref())?;
    for (name, contents) in gece_states(lang) {
        write_png(
            &dir.join(format!("{name}.png")),
            &cell(&contents, Look::gece())?,
        )?;
    }
    let view = audio_view(&fixtures::lit(&fixtures::banana()), PickMode::None, lang);
    // The three retired themes, as the settings migration turns them into Gece picks.
    let looks = [
        (
            "kagit",
            Look::picked(
                Some([0xF3, 0xF2, 0xED]),
                [0x3F, 0x6E, 0x00],
                hex(0xC9, 0xCD, 0xD3),
            ),
        ),
        (
            "york",
            Look::picked(
                Some([0x1C, 0x24, 0x20]),
                [0xFF, 0xE9, 0xA8],
                hex(0x14, 0x18, 0x1A),
            ),
        ),
        (
            "sodyum",
            Look::picked(None, [0xFF, 0xB2, 0x24], hex(0x1A, 0x1A, 0x1D)),
        ),
    ];
    for (name, look) in looks {
        let page = audio_page(&view, fixtures::CLOCK, lang);
        let contents = [Content::Page(PageId::Audio, page)];
        write_png(
            &dir.join(format!("12-theme-{name}.png")),
            &cell(&contents, look)?,
        )?;
    }
    println!("{}", dir.display());
    Ok(())
}
