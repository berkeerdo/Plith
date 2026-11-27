//! Decoding a material pack's rasters once per scale, at the size that matches the display.
//!
//! The bytes are the pack's own embedded files (`BuiltinPack::files`); nothing is embedded
//! twice. Decoding runs on the UI thread, which owns COM, and only when the drawn theme has a
//! material.

use plith_core::draw::{ArtId, ArtPixels};
use plith_core::material::art::MaterialArt;
use plith_core::theme::builtin::{BUILTIN, BuiltinPack};
use plith_core::theme::pack::parse_pack;
use plith_core::theme::registry::ThemeId;
use std::sync::Arc;

/// From this many pixels per DIP on, the 2x files are used.
const TWO_X_FROM: f64 = 1.5;

/// Whether `scale` (pixels per DIP) draws from the 2x files.
#[must_use]
pub fn two_x(scale: f64) -> bool {
    scale >= TWO_X_FROM
}

/// The built-in pack with manifest id `id`.
#[must_use]
pub fn builtin_pack(id: &ThemeId) -> Option<&'static BuiltinPack> {
    BUILTIN
        .iter()
        .find(|b| parse_pack(b.manifest).is_ok_and(|p| p.id == id.as_str()))
}

/// The rasters of `pack` (id `id`) from the 1x or 2x files by `scale`. Each picture keeps its
/// own pixel size. Needs COM on the calling thread.
///
/// # Errors
/// `theme art: <pack id> <file>` for the first file that is missing or does not decode.
pub fn load(id: &str, pack: &BuiltinPack, scale: f64) -> Result<Arc<MaterialArt>, String> {
    let suffix = if two_x(scale) { "2x" } else { "1x" };
    let get = |base: &str| decode(id, pack, &format!("{base}-{suffix}"));
    Ok(Arc::new(MaterialArt {
        fabric: get("floor")?,
        rings: [get("ring-1")?, get("ring-2")?, get("ring-3")?],
        disc: get("disc")?,
    }))
}

/// One file at its own size (a longest-side cap of `u32::MAX` never scales).
fn decode(id: &str, pack: &BuiltinPack, file: &str) -> Result<Arc<ArtPixels>, String> {
    let fail = || format!("theme art: {id} {file}");
    let bytes = pack
        .files
        .iter()
        .find(|(name, _)| *name == file)
        .map(|(_, bytes)| *bytes)
        .ok_or_else(fail)?;
    let art = plith_media::art::decode_image(bytes, u32::MAX).ok_or_else(fail)?;
    Ok(Arc::new(ArtPixels {
        id: ArtId(art.id),
        width: art.width,
        height: art.height,
        bgra: art.bgra,
    }))
}

/// The decoded art of the drawn theme, kept while the theme and the scale class stay the same,
/// so a palette-only switch reuses the same `Arc`.
#[derive(Debug, Default)]
pub struct ArtCache {
    /// The pack id, the file scale class and the art.
    held: Option<(ThemeId, bool, Arc<MaterialArt>)>,
}

impl ArtCache {
    /// The art of pack `drawn` at `scale`: the held one when it matches, else decoded now.
    ///
    /// # Errors
    /// The message of [`load`]; also when `drawn` is not a built-in pack with files.
    pub fn get(&mut self, drawn: &ThemeId, scale: f64) -> Result<Arc<MaterialArt>, String> {
        let class = two_x(scale);
        if let Some((id, held_class, art)) = &self.held
            && id == drawn
            && *held_class == class
        {
            return Ok(Arc::clone(art));
        }
        let pack = builtin_pack(drawn).ok_or_else(|| format!("theme art: {drawn:?} no pack"))?;
        let art = load(drawn.as_str(), pack, scale)?;
        self.held = Some((drawn.clone(), class, Arc::clone(&art)));
        Ok(art)
    }
}

#[cfg(test)]
mod tests;
