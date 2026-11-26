//! The Halka review set (`--halka <new folder>`), at 1x and 2x: the collapsed pill looks Karot,
//! Clock, Playing and Hidden; the audio, media and welcome pages open; the expand strip on the
//! welcome page; the settings window; and the same collapsed and open frames in Gece beside them.
//!
//! The strip frames are real frames of a BRISK expand from the Karot pill to the welcome page,
//! stepped at 240 Hz the way the render loop steps them: each frame is the first one whose morph
//! progress reaches 0, 0.25, 0.40, 0.55, 0.70 and 1.0, with the island shape, the content
//! opacity and the ripple time that frame has.

use super::{
    BOTTOM, CELL_W, Canvas, EDGE, PILL_GAP, SCALE_CELL, idle_states, png, px, sheet_facts,
};
use plith_core::draw::{ArtId, ArtPixels, DisplayList, PageContent};
use plith_core::geometry::IslandShape;
use plith_core::island::IslandMotion;
use plith_core::lang::Lang;
use plith_core::material::art::{MaterialArt, ThemePaint};
use plith_core::material::{EXPAND_SETTLE_MS, MaterialLayers, MaterialMotion, morph_progress};
use plith_core::page::{PageLayer, PageLayers};
use plith_core::pages::audio::{PickMode, audio_page, audio_view};
use plith_core::pages::fixtures;
use plith_core::pages::idle_pill::{idle_pill, rest_shape};
use plith_core::pages::media::media_page;
use plith_core::pages::welcome::welcome_page;
use plith_core::pages::{PageBuild, PageId};
use plith_core::settings::IdleLook;
use plith_core::shapes::REST;
use plith_core::sheet::{ICON_ART, ICON_DIP, Section, SheetView, sheet_build};
use plith_core::spring::Spring;
use plith_core::theme::builtin::{BUILTIN, BuiltinPack};
use plith_core::theme::pack::parse_pack;
use plith_core::theme::registry::{Picks, Registry, ThemeId};
use plith_core::theme::{Palette, hex};
use plith_core::tuning::BRISK;
use plith_render::offscreen::Offscreen;
use plith_render::paint::PaintFrame;
use plith_render::{TextSystem, render_sheet_offscreen};
use std::path::Path;
use std::sync::Arc;

type Error = Box<dyn std::error::Error>;

/// The language of the set.
const LANG: Lang = Lang::Tr;
/// The morph progress of each strip frame.
const STRIP: [f64; 6] = [0.0, 0.25, 0.40, 0.55, 0.70, 1.0];
/// One frame at 240 Hz, in seconds.
const FRAME_S: f64 = 1.0 / 240.0;
/// Progress this close to 1 counts as the open end (the spring approaches it).
const OPEN_EPS: f64 = 1e-3;

/// One island: its shape, what it shows and how far the material has moved.
struct Shot {
    shape: IslandShape,
    content: Arc<PageContent>,
    pages: Vec<PageLayer>,
    opacity: f32,
    layers: MaterialLayers,
}

fn page_shot(id: PageId, build: &PageBuild) -> Shot {
    let mut content = PageContent::empty();
    content.art = vec![Arc::new(fixtures::art())];
    content.pages[id.index()] = Arc::new(build.list.clone());
    Shot {
        shape: build.shape,
        content: Arc::new(content),
        pages: vec![PageLayer {
            page: id.index(),
            offset: 0.0,
            opacity: 1.0,
        }],
        opacity: 1.0,
        layers: MaterialLayers::OPEN,
    }
}

fn pill_shot(list: DisplayList, shape: IslandShape) -> Shot {
    let mut content = PageContent::empty();
    content.pill = Arc::new(list);
    Shot {
        shape,
        content: Arc::new(content),
        pages: Vec::new(),
        opacity: 0.0,
        layers: MaterialLayers::COLLAPSED,
    }
}

fn halka_pack() -> Result<&'static BuiltinPack, Error> {
    BUILTIN
        .iter()
        .find(|b| parse_pack(b.manifest).is_ok_and(|p| p.id == "halka"))
        .ok_or_else(|| "halka is not built in".into())
}

fn decode(pack: &BuiltinPack, name: &str) -> Result<Arc<ArtPixels>, Error> {
    let bytes = pack
        .files
        .iter()
        .find(|(file, _)| *file == name)
        .ok_or_else(|| format!("no file {name}"))?
        .1;
    let art = plith_media::art::decode_image(bytes, u32::MAX)
        .ok_or_else(|| format!("{name} does not decode"))?;
    Ok(Arc::new(ArtPixels {
        id: ArtId(art.id),
        width: art.width,
        height: art.height,
        bgra: art.bgra,
    }))
}

fn halka_id() -> ThemeId {
    ThemeId::parse("halka").unwrap_or_default()
}

/// The Halka paint as the app sends it at `scale`: the 2x files from 1.5 on.
fn halka_paint(scale: f64) -> Result<ThemePaint, Error> {
    let pack = halka_pack()?;
    let suffix = if scale >= 1.5 { "2x" } else { "1x" };
    let get = |base: &str| decode(pack, &format!("{base}-{suffix}"));
    let art = MaterialArt {
        fabric: get("floor")?,
        rings: [get("ring-1")?, get("ring-2")?, get("ring-3")?],
        disc: get("disc")?,
        pill: get("pill")?,
    };
    let picks = Picks {
        accent: None,
        floor: None,
        windows_accent: None,
    };
    let resolved = Registry::builtin().resolve(&halka_id(), picks);
    Ok(ThemePaint::new(
        halka_id(),
        resolved.open,
        resolved.collapsed,
        Some(Arc::new(art)),
    ))
}

/// Paints one shot at its own size; returns the rows and the pixel size.
fn paint_shot(shot: &Shot, paint: &ThemePaint) -> Result<(Vec<u8>, usize, usize), Error> {
    let frame = PaintFrame {
        shape: shot.shape,
        origin: (0.0, 0.0),
        pages: PageLayers::of(&shot.pages),
        content_opacity: shot.opacity,
        content: Arc::clone(&shot.content),
        content_gen: 1,
        live: fixtures::live_values(),
        material: shot.layers,
    };
    let (w, h) = (px(shot.shape.width), px(shot.shape.height));
    let (w32, h32) = (u32::try_from(w)?, u32::try_from(h)?);
    let mut target = Offscreen::new(w32, h32, super::scale(), paint.open, TextSystem::new()?)?;
    target.set_theme(paint.clone());
    Ok((target.render(&frame)?, w, h))
}

/// The shots side by side, centred, flush under the screen edge on the desk.
fn row(shots: &[Shot], paint: &ThemePaint) -> Result<Canvas, Error> {
    let gaps = f64::from(u32::try_from(shots.len().saturating_sub(1))?);
    let row_w: f64 = shots.iter().map(|s| s.shape.width).sum::<f64>() + PILL_GAP * gaps;
    let tallest = shots.iter().map(|s| s.shape.height).fold(0.0, f64::max);
    let width = CELL_W.max(row_w + 2.0 * PILL_GAP);
    let mut canvas = Canvas::desk(width, EDGE + tallest + BOTTOM, hex(0x1B, 0x1D, 0x21));
    let mut x = (width - row_w) / 2.0;
    for shot in shots {
        let (rows, w, h) = paint_shot(shot, paint)?;
        canvas.over(&rows, w, h, px(x), px(EDGE));
        x += shot.shape.width + PILL_GAP;
    }
    Ok(canvas)
}

fn collapsed(look: IdleLook) -> Shot {
    let facts = idle_states::idle_facts(LANG);
    pill_shot(idle_pill(look, &facts), rest_shape(look))
}

/// The collapsed and open states, by file name.
fn states() -> Vec<(&'static str, Shot)> {
    let clock = fixtures::CLOCK;
    let view = audio_view(&fixtures::lit(&fixtures::banana()), PickMode::None, LANG);
    let welcome = welcome_page(&idle_states::rest(LANG), None);
    vec![
        ("pill-karot", collapsed(IdleLook::Karot)),
        ("pill-clock", collapsed(IdleLook::Clock)),
        ("pill-playing", collapsed(IdleLook::Playing)),
        ("pill-hidden", collapsed(IdleLook::Hidden)),
        (
            "open-audio",
            page_shot(PageId::Audio, &audio_page(&view, clock, LANG)),
        ),
        (
            "open-media",
            page_shot(
                PageId::Media,
                &media_page(&fixtures::media_playing(), clock, LANG),
            ),
        ),
        ("open-welcome", page_shot(PageId::Welcome, &welcome)),
    ]
}

/// The strip: a BRISK expand from the Karot pill to the welcome page, one shot per `STRIP`
/// progress, with the ripple time in ms. The ripples run on the frame's own time, or with
/// `plan_clock` on `progress x EXPAND_SETTLE_MS` (the plan's reading, where progress grows
/// linearly with time; the BRISK spring opens much faster than that).
fn strip(plan_clock: bool) -> Vec<(f64, f64, Shot)> {
    let facts = idle_states::idle_facts(LANG);
    let from = rest_shape(IdleLook::Karot);
    let welcome = welcome_page(&idle_states::rest(LANG), None);
    let to = welcome.shape;
    let mut island = IslandMotion::new(from, BRISK);
    island.retarget(to);
    let mut opacity = Spring::new(0.0, BRISK.opacity).with_eps(0.002, 0.01);
    opacity.retarget(1.0, BRISK.opacity);
    let mut material = MaterialMotion::new();
    material.expand(0.0, 0.0);
    let mut out = Vec::new();
    let mut t_ms = 0.0;
    while out.len() < STRIP.len() && t_ms < 2000.0 {
        let progress = morph_progress(island.shape().height, REST.height, to.height);
        let want = STRIP[out.len()];
        if progress >= want.min(1.0 - OPEN_EPS) {
            let mut shot = page_shot(PageId::Welcome, &welcome);
            let mut content = (*shot.content).clone();
            content.pill = Arc::new(idle_pill(IdleLook::Karot, &facts));
            shot.content = Arc::new(content);
            shot.shape = island.shape();
            #[allow(clippy::cast_possible_truncation)]
            // An opacity in 0..=1.
            let value = opacity.value().clamp(0.0, 1.0) as f32;
            shot.opacity = value;
            let ripple_ms = if plan_clock {
                want * EXPAND_SETTLE_MS
            } else {
                t_ms
            };
            shot.layers = material.layers(ripple_ms, progress);
            out.push((want, ripple_ms, shot));
            continue;
        }
        island.step(FRAME_S);
        opacity.step(FRAME_S);
        t_ms += FRAME_S * 1000.0;
    }
    out
}

/// Decodes the About icon tile at `side` pixels, as `render_sheet` does.
fn load_icon(side: u32) -> Result<Arc<ArtPixels>, Error> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../plith/assets/plith-tile-256.png");
    let (width, height, pixels) = png::decode_png(&path, Some(side))?;
    Ok(Arc::new(ArtPixels {
        id: ICON_ART,
        width,
        height,
        bgra: Arc::from(pixels),
    }))
}

/// The settings window in Halka (its sheet station: Gece's floor), Look and General sections.
fn render_settings(dir: &Path, factor: f64) -> Result<(), Error> {
    let picks = Picks {
        accent: None,
        floor: None,
        windows_accent: None,
    };
    let palette = Registry::builtin().resolve(&halka_id(), picks).sheet;
    let mut facts = sheet_facts::sheet_facts(LANG);
    facts.settings.theme = halka_id();
    for (name, section) in [("look", Section::Look), ("general", Section::General)] {
        let view = SheetView {
            section,
            ..SheetView::default()
        };
        let (list, _) = sheet_build(&facts, &view);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        // The icon is a few dozen pixels.
        let icon_px = (ICON_DIP * factor).round().min(256.0) as u32;
        let rows = render_sheet_offscreen(&list, factor, palette, &[load_icon(icon_px)?])?;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        // The sheet is 840 by 640 DIP.
        let size = |dip: f64| (dip * factor).ceil() as u32;
        let file = dir.join(format!("halka-settings-{name}-{factor}x.png"));
        png::write_png(&file, &rows, size(840.0), size(640.0))?;
    }
    Ok(())
}

/// Writes the whole set at 1x and 2x into the new folder `dir`; prints the strip timing.
pub(super) fn render(dir: &Path) -> Result<(), Error> {
    let dir = super::out_dir(Some(dir))?;
    let gece = ThemePaint::plain(Palette::default());
    for factor in [1.0, 2.0] {
        SCALE_CELL.with(|s| s.set(factor));
        let halka = halka_paint(factor)?;
        for (name, shot) in states() {
            for (theme, paint) in [("halka", &halka), ("gece", &gece)] {
                let file = dir.join(format!("{theme}-{name}-{factor}x.png"));
                super::write_png(&file, &row(std::slice::from_ref(&shot), paint)?)?;
            }
        }
        render_strip(&dir, factor, &halka, "strip", false)?;
        render_strip(&dir, factor, &halka, "strip-plan", true)?;
        render_settings(&dir, factor)?;
    }
    println!("{}", dir.display());
    Ok(())
}

/// Writes one strip (`name`, see [`strip`]) and prints each frame's numbers.
fn render_strip(
    dir: &Path,
    factor: f64,
    halka: &ThemePaint,
    name: &str,
    plan_clock: bool,
) -> Result<(), Error> {
    for (progress, ripple_ms, shot) in strip(plan_clock) {
        let file = dir.join(format!(
            "halka-{name}-{:03.0}-{factor}x.png",
            progress * 100.0
        ));
        let l = shot.layers;
        println!(
            "{name} {factor}x {progress:.2}: ripple clock {ripple_ms:.1} ms, {:.1}x{:.1}, \
             content {:.2}, reveal {:.2}, ripples {:.2}/{:.2}/{:.2}, disc {:.2} lift {:.1}, \
             pill {:.2}",
            shot.shape.width,
            shot.shape.height,
            shot.opacity,
            l.reveal,
            l.ripples[0],
            l.ripples[1],
            l.ripples[2],
            l.disc,
            l.disc_lift,
            l.pill
        );
        super::write_png(&file, &row(std::slice::from_ref(&shot), halka)?)?;
    }
    Ok(())
}
