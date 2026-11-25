//! Renders every installer screen offscreen on WARP, in Turkish and English, at scale 1 and 2,
//! and writes one PNG each (`<screen>-<lang>-<scale>x.png`) for review against the drawn comps
//! (`docs/design/identity/r3/a1-start.png` to `a3-done.png`). No window opens.
//!
//! Run: `cargo run --release -p plith-render --example render_setup -- <new folder>`. The folder
//! must not exist yet; its parents are made. `--comps` after the folder renders only the start,
//! running (step 1 pressed) and done screens in Turkish, the three that have a comp.

#[path = "common/png.rs"]
mod png;

use plith_core::draw::ArtPixels;
use plith_core::lang::Lang;
use plith_core::setup::{BG_ART, BUTTON_ART, Screen, Version, setup_build};
use plith_core::theme::Palette;
use plith_render::render_sheet_offscreen;
use png::{decode_png, write_png};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};
use windows::core::Result;

/// This setup's version, drawn top left.
const OWN: Version = Version::new(0, 5, 0);

/// Loads one installer material file at its own pixel size (no rescale).
fn load_art(id: plith_core::draw::ArtId, file: &str) -> Result<Arc<ArtPixels>> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/design/identity/r3/installer")
        .join(file);
    let (width, height, pixels) = decode_png(&path, None)?;
    Ok(Arc::new(ArtPixels {
        id,
        width,
        height,
        bgra: Arc::from(pixels),
    }))
}

/// The four backgrounds and the button from the 1x or 2x files.
fn art_set(scale: u32) -> Result<Vec<Arc<ArtPixels>>> {
    let mut art = Vec::new();
    for (n, id) in BG_ART.iter().enumerate() {
        art.push(load_art(*id, &format!("bg-{n}-{scale}x.png"))?);
    }
    art.push(load_art(BUTTON_ART, &format!("button-{scale}x.png"))?);
    Ok(art)
}

/// Every screen with the name its file carries.
fn screens() -> Vec<(&'static str, Screen)> {
    vec![
        ("start", Screen::Start),
        ("running", Screen::Running { done: 1 }),
        ("done", Screen::Done { autostart: true }),
        (
            "error",
            Screen::Error {
                step: 2,
                reason: "The helper could not be started (error 1223).".into(),
            },
        ),
        (
            "update",
            Screen::Update {
                from: Version::new(0, 4, 0),
                to: Version::new(0, 5, 0),
            },
        ),
        (
            "newer",
            Screen::Newer {
                installed: Version::new(0, 5, 0),
            },
        ),
        (
            "uninstall-confirm",
            Screen::UninstallConfirm { delete_data: false },
        ),
        ("uninstalling", Screen::Uninstalling { done: 1 }),
        ("uninstalled", Screen::Uninstalled),
        ("uac-cancelled", Screen::UacCancelled),
    ]
}

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let dir: PathBuf = args
        .next()
        .ok_or("usage: render_setup <new folder> [--comps]")?
        .into();
    let comps_only = match args.next() {
        None => false,
        Some(flag) if flag == "--comps" => true,
        Some(other) => return Err(format!("unknown option {}", other.to_string_lossy()).into()),
    };
    if let Some(parent) = dir.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::create_dir(&dir)?;
    // SAFETY: called once on the main thread before any COM use.
    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok()? };
    let palette = Palette::setup();
    let langs: &[Lang] = if comps_only {
        &[Lang::Tr]
    } else {
        &[Lang::Tr, Lang::En]
    };
    for scale in [1u32, 2] {
        let art = art_set(scale)?;
        for (name, screen) in screens() {
            if comps_only && !["start", "running", "done"].contains(&name) {
                continue;
            }
            for &lang in langs {
                let (list, _) = setup_build(&screen, OWN, lang);
                let rows = render_sheet_offscreen(&list, f64::from(scale), palette, &art)?;
                let file = format!("{name}-{}-{scale}x.png", lang.code());
                write_png(&dir.join(file), &rows, 640 * scale, 420 * scale)?;
            }
        }
    }
    println!("{}", dir.display());
    Ok(())
}
