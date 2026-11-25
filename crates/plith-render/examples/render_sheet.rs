//! Renders every settings sheet section offscreen on WARP, in Turkish and German, at scale 1
//! and 2, and writes one PNG each (`<section>-<lang>-<scale>x.png`) for review against the drawn
//! Idle section (`docs/design/r7/renders-3/sheet.png`). No window opens.
//!
//! Run: `cargo run --release -p plith-render --example render_sheet -- <new folder>`. The folder
//! must not exist yet; its parents are made. `--theme <key>` after the folder paints every
//! section in that theme (default `gece`), with the same theme marked in the Look section.
//! `--update current|checking|ready|downloading|signature|failed` shows that state in the About
//! update row, `--sections general,about` renders only those sections and `--lang tr` only that
//! language; each flag may follow the folder in any order.

#[path = "render_sheet/facts.rs"]
mod facts;
#[path = "common/png.rs"]
mod png;

use plith_core::draw::ArtPixels;
use plith_core::lang::Lang;
use plith_core::setup::Version;
use plith_core::sheet::{ICON_ART, ICON_DIP, Section, SheetView, sheet_build};
use plith_core::theme::Theme;
use plith_core::update::UpdateRow;
use plith_render::render_sheet_offscreen;
use png::{decode_png, write_png};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};
use windows::core::Result;

/// Decodes the About icon tile (`crates/plith/assets/plith-tile-256.png`) to premultiplied BGRA,
/// scaled to the drawn size with Fant, as the app does, so the render matches it.
fn load_icon(side: u32) -> Result<Arc<ArtPixels>> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../plith/assets/plith-tile-256.png");
    let (width, height, pixels) = decode_png(&path, Some(side))?;
    Ok(Arc::new(ArtPixels {
        id: ICON_ART,
        width,
        height,
        bgra: Arc::from(pixels),
    }))
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

/// What the optional flags after the folder chose.
struct Options {
    theme: Theme,
    update: UpdateRow,
    sections: Vec<Section>,
    langs: Vec<Lang>,
}

fn section_of(key: &str) -> Option<Section> {
    Section::ALL.into_iter().find(|s| {
        section_name(*s)
            .split_once('-')
            .is_some_and(|(_, name)| name == key)
    })
}

fn update_of(key: &str) -> Option<UpdateRow> {
    Some(match key {
        "current" => UpdateRow::Current,
        "checking" => UpdateRow::Checking,
        "ready" => UpdateRow::Ready(Version::new(0, 5, 0)),
        "downloading" => UpdateRow::Downloading(42),
        "signature" => UpdateRow::BadSignature,
        "failed" => UpdateRow::Failed,
        _ => return None,
    })
}

fn options(
    mut args: impl Iterator<Item = std::ffi::OsString>,
) -> std::result::Result<Options, String> {
    let mut out = Options {
        theme: Theme::Gece,
        update: UpdateRow::Current,
        sections: Section::ALL.to_vec(),
        langs: vec![Lang::Tr, Lang::De],
    };
    while let Some(flag) = args.next() {
        let flag = flag.to_string_lossy().into_owned();
        let value = args
            .next()
            .ok_or_else(|| format!("{flag} needs a value"))?
            .to_string_lossy()
            .into_owned();
        match flag.as_str() {
            "--theme" => {
                out.theme = Theme::from_key(&value).ok_or(format!("unknown theme {value}"))?;
            }
            "--update" => {
                out.update = update_of(&value).ok_or(format!("unknown update state {value}"))?;
            }
            "--sections" => {
                out.sections = value
                    .split(',')
                    .map(|key| section_of(key).ok_or(format!("unknown section {key}")))
                    .collect::<std::result::Result<_, _>>()?;
            }
            "--lang" => {
                let lang = Lang::ALL.into_iter().find(|l| l.code() == value);
                out.langs = vec![lang.ok_or(format!("unknown language {value}"))?];
            }
            _ => return Err(format!("unknown option {flag}")),
        }
    }
    Ok(out)
}

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let dir: PathBuf = args
        .next()
        .ok_or("usage: render_sheet <new folder> [--theme key] [--update state] [--sections list] [--lang code]")?
        .into();
    let Options {
        theme,
        update,
        sections,
        langs,
    } = options(args)?;
    if let Some(parent) = dir.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::create_dir(&dir)?;
    // SAFETY: called once on the main thread before any COM use.
    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok()? };
    let palette = theme.palette();

    for lang in langs {
        let mut facts = facts::sheet_facts(lang);
        facts.update = update;
        facts.settings.theme = theme;
        // Turkish shows the Windows start switch off, German on, so one run covers both.
        facts.autostart = lang == Lang::De;
        for &section in &sections {
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
