//! Renders every page state offscreen on WARP at scale 2 and writes PNGs for review against the
//! R3 mock: each island sits flush under a black screen edge on a desk cell, like the mock's
//! cells. Output goes to a new folder `%TEMP%\plith-renders-<unix seconds>-<pid>`; no window
//! opens.
//!
//! Run: `cargo run --release -p plith-render --example render_pages`. With a folder argument
//! (`-- <new folder>`) it renders only the idle look states of the R7 mock (the welcome view and
//! the six pill looks, from a fixed day and time) into that folder, which must not exist yet.

use plith_core::draw::{DisplayList, PageContent};
use plith_core::geometry::IslandShape;
use plith_core::page::{PageLayer, PageLayers};
use plith_core::pages::audio::{PickMode, audio_page, audio_view};
use plith_core::pages::fixtures;
use plith_core::pages::media::media_page;
use plith_core::pages::pill::pill;
use plith_core::pages::shelf::shelf_page;
use plith_core::pages::system::system_page;
use plith_core::pages::{PageBuild, PageId};
use plith_core::shapes::REST;
use plith_core::theme::{Palette, Theme, hex};
use plith_render::paint::PaintFrame;
use plith_render::{TextSystem, render_offscreen};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
#[path = "render_pages/idle_states.rs"]
mod idle_states;

use windows::Win32::Foundation::GENERIC_WRITE;
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_ContainerFormatPng, GUID_WICPixelFormat32bppBGRA,
    IWICBitmapFrameEncode, IWICImagingFactory, WICBitmapEncoderNoCache,
};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
};
use windows::core::{HSTRING, Result};

/// Pixels per DIP.
const SCALE: f64 = 2.0;
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
    (dip * SCALE).ceil() as usize
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
    };
    let (w, h) = (px(shape.width), px(shape.height));
    let text = TextSystem::new()?;
    #[allow(clippy::cast_possible_truncation)]
    // Island sizes are a few hundred DIP.
    let rows = render_offscreen(w as u32, h as u32, SCALE, &frame, palette, text)?;
    Ok((rows, w, h))
}

/// The desk colour of each theme's mock cell.
fn desk(theme: Theme) -> [f32; 4] {
    match theme {
        Theme::Gece => hex(0x1B, 0x1D, 0x21),
        Theme::Kagit => hex(0xC9, 0xCD, 0xD3),
        Theme::York => hex(0x14, 0x18, 0x1A),
        Theme::Sodyum => hex(0x1A, 0x1A, 0x1D),
    }
}

fn shape_of(content: &Content) -> IslandShape {
    match content {
        Content::Page(_, build) => build.shape,
        Content::Pill(_, shape) => *shape,
    }
}

/// A cell with the islands side by side, centred, flush under the edge.
fn cell(contents: &[Content], theme: Theme) -> Result<Canvas> {
    let shapes: Vec<IslandShape> = contents.iter().map(shape_of).collect();
    let row_w: f64 = shapes.iter().map(|s| s.width).sum::<f64>()
        + PILL_GAP * f64::from(u32::try_from(shapes.len()).unwrap_or(1).saturating_sub(1));
    let tallest = shapes.iter().map(|s| s.height).fold(0.0, f64::max);
    let mut canvas = Canvas::desk(CELL_W, EDGE + tallest + BOTTOM, desk(theme));
    let mut x = (CELL_W - row_w) / 2.0;
    for (content, shape) in contents.iter().zip(&shapes) {
        let (rows, w, h) = island(content, theme.palette())?;
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
fn gece_states() -> Vec<(&'static str, Vec<Content>)> {
    let clock = fixtures::CLOCK;
    let audio = |facts, mode| {
        let view = audio_view(&fixtures::lit(&facts), mode);
        vec![Content::Page(PageId::Audio, audio_page(&view, clock))]
    };
    let media = |facts| vec![Content::Page(PageId::Media, media_page(&facts, clock))];
    let system = |facts| vec![Content::Page(PageId::System, system_page(&facts, clock))];
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
            vec![Content::Page(PageId::Shelf, shelf_page(clock))],
        ),
        ("08-system", system(fixtures::system())),
        ("09-system-mic-muted", system(fixtures::system_mic_muted())),
        ("10-system-no-ddc", system(fixtures::system_no_ddc())),
        (
            "11-pills",
            fixtures::pills()
                .iter()
                .map(|p| Content::Pill(pill(p), REST))
                .collect(),
        ),
    ]
}

/// A new, uniquely named output folder under `%TEMP%`.
fn out_dir() -> std::io::Result<PathBuf> {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let dir = std::env::temp_dir().join(format!("plith-renders-{secs}-{}", std::process::id()));
    std::fs::create_dir(&dir)?;
    Ok(dir)
}

/// Renders the idle look states into the new folder `dir`.
fn render_idle(dir: &Path) -> std::result::Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir(dir)?;
    for (name, contents) in idle_states::states() {
        write_png(
            &dir.join(format!("{name}.png")),
            &cell(&contents, Theme::Gece)?,
        )?;
    }
    println!("{}", dir.display());
    Ok(())
}

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // SAFETY: called once on the main thread before any COM use.
    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok()? };
    if let Some(dir) = std::env::args_os().nth(1) {
        return render_idle(Path::new(&dir));
    }
    let dir = out_dir()?;
    for (name, contents) in gece_states() {
        write_png(
            &dir.join(format!("{name}.png")),
            &cell(&contents, Theme::Gece)?,
        )?;
    }
    let view = audio_view(&fixtures::lit(&fixtures::banana()), PickMode::None);
    let themes = [
        ("kagit", Theme::Kagit),
        ("york", Theme::York),
        ("sodyum", Theme::Sodyum),
    ];
    for (name, theme) in themes {
        let page = audio_page(&view, fixtures::CLOCK);
        let contents = [Content::Page(PageId::Audio, page)];
        write_png(
            &dir.join(format!("12-theme-{name}.png")),
            &cell(&contents, theme)?,
        )?;
    }
    println!("{}", dir.display());
    Ok(())
}
