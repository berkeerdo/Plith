//! Renders every settings sheet section offscreen on WARP, in Turkish and German, at scale 1
//! and 2, and writes one PNG each (`<section>-<lang>-<scale>x.png`) for review against the drawn
//! Idle section (`docs/design/r7/renders-3/sheet.png`). No window opens.
//!
//! Run: `cargo run --release -p plith-render --example render_sheet -- <new folder>`. The folder
//! must not exist yet; its parents are made. `--theme <key>` after the folder paints every
//! section in that theme (default `gece`), with the same theme marked in the Look section.

#[path = "render_sheet/facts.rs"]
mod facts;

use plith_core::draw::ArtPixels;
use plith_core::lang::Lang;
use plith_core::sheet::{ICON_ART, ICON_DIP, Section, SheetView, sheet_build};
use plith_core::theme::Theme;
use plith_render::render_sheet_offscreen;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use windows::Win32::Foundation::{GENERIC_READ, GENERIC_WRITE};
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_ContainerFormatPng, GUID_WICPixelFormat32bppBGRA,
    GUID_WICPixelFormat32bppPBGRA, IWICBitmapFrameEncode, IWICImagingFactory, IWICPalette,
    WICBitmapDitherTypeNone, WICBitmapEncoderNoCache, WICBitmapInterpolationModeFant,
    WICBitmapPaletteTypeCustom, WICDecodeMetadataCacheOnDemand,
};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
};
use windows::core::{HSTRING, Result};

/// Writes `w` by `h` BGRA rows as a PNG with WIC.
fn write_png(path: &Path, rows: &[u8], w: u32, h: u32) -> Result<()> {
    // SAFETY: COM is initialized on this thread before the factory is created; every pointer
    // passed points to a live local, and `rows` holds `h` rows of `w * 4` bytes.
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
        frame.WritePixels(h, w * 4, rows)?;
        frame.Commit()?;
        encoder.Commit()?;
    }
    Ok(())
}

/// Decodes the About icon tile (`crates/plith/assets/plith-tile-256.png`) to premultiplied BGRA.
fn load_icon(side: u32) -> Result<Arc<ArtPixels>> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../plith/assets/plith-tile-256.png");
    // SAFETY: COM is initialized on this thread; every out pointer is a live local.
    unsafe {
        let factory: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
        let decoder = factory.CreateDecoderFromFilename(
            &HSTRING::from(path.as_os_str()),
            None,
            GENERIC_READ,
            WICDecodeMetadataCacheOnDemand,
        )?;
        let frame = decoder.GetFrame(0)?;
        // Scaled to the drawn size with Fant, as the app does, so the render matches it.
        let scaler = factory.CreateBitmapScaler()?;
        scaler.Initialize(&frame, side, side, WICBitmapInterpolationModeFant)?;
        let converter = factory.CreateFormatConverter()?;
        converter.Initialize(
            &scaler,
            &GUID_WICPixelFormat32bppPBGRA,
            WICBitmapDitherTypeNone,
            None::<&IWICPalette>,
            0.0,
            WICBitmapPaletteTypeCustom,
        )?;
        let (mut w, mut h) = (0u32, 0u32);
        converter.GetSize(&raw mut w, &raw mut h)?;
        let mut pixels = vec![0u8; (w * h * 4) as usize];
        converter.CopyPixels(std::ptr::null(), w * 4, &mut pixels)?;
        Ok(Arc::new(ArtPixels {
            id: ICON_ART,
            width: w,
            height: h,
            bgra: Arc::from(pixels),
        }))
    }
}

fn section_name(section: Section) -> &'static str {
    match section {
        Section::General => "1-general",
        Section::Look => "1b-look",
        Section::Idle => "2-idle",
        Section::Osd => "3-osd",
        Section::About => "4-about",
    }
}

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let dir: PathBuf = args
        .next()
        .ok_or("usage: render_sheet <new folder> [--theme gece|kagit|york|sodyum]")?
        .into();
    let theme = match args.next() {
        None => Theme::Gece,
        Some(flag) if flag == "--theme" => {
            let key = args.next().ok_or("--theme needs a key")?;
            let key = key.to_string_lossy();
            Theme::from_key(&key).ok_or_else(|| format!("unknown theme `{key}`"))?
        }
        Some(other) => return Err(format!("unknown option {}", other.to_string_lossy()).into()),
    };
    if let Some(parent) = dir.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::create_dir(&dir)?;
    // SAFETY: called once on the main thread before any COM use.
    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok()? };
    let palette = theme.palette();

    for lang in [Lang::Tr, Lang::De] {
        let mut facts = facts::sheet_facts(lang);
        facts.settings.theme = theme;
        // Turkish shows the Windows start switch off, German on, so one run covers both.
        facts.autostart = lang == Lang::De;
        for section in Section::ALL {
            let view = SheetView {
                section,
                ..SheetView::default()
            };
            let (list, _) = sheet_build(&facts, &view);
            // The About icon is also checked at 250 %, where the 256 px tile is scaled up.
            let scales: &[f64] = if section == Section::About {
                &[1.0, 2.0, 2.5]
            } else {
                &[1.0, 2.0]
            };
            for &scale in scales {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let side = (ICON_DIP * scale).round().min(256.0) as u32;
                let rows = render_sheet_offscreen(&list, scale, palette, &[load_icon(side)?])?;
                let name = format!("{}-{}-{scale}x.png", section_name(section), lang.code());
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let px = |dip: f64| (dip * scale).ceil() as u32;
                write_png(&dir.join(name), &rows, px(840.0), px(640.0))?;
            }
        }
    }
    println!("{}", dir.display());
    Ok(())
}
