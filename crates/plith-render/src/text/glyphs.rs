//! A test hook: which characters a style's bundled face has no glyph for.

use plith_core::draw::{TextStyle, style_spec};
use windows::Win32::Foundation::E_FAIL;
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT,
};
use windows::core::{BOOL, Error, HSTRING, Result};

use super::TextSystem;

impl TextSystem {
    /// Test hook: the characters of `text` (upper-cased when the style is) that the bundled face
    /// matched for `style` has no glyph for, or `None` when the system fonts are in use. An
    /// empty list means every character draws from the bundled face.
    ///
    /// # Errors
    /// Returns the failing DirectWrite call.
    #[doc(hidden)]
    pub fn missing_glyphs(&self, text: &str, style: TextStyle) -> Result<Option<Vec<char>>> {
        let Some(collection) = &self.fonts.collection else {
            return Ok(None);
        };
        let spec = style_spec(style);
        let shown: Vec<char> = if spec.uppercase {
            text.to_uppercase().chars().collect()
        } else {
            text.chars().collect()
        };
        let points: Vec<u32> = shown.iter().map(|c| u32::from(*c)).collect();
        let mut indices = vec![0u16; points.len()];
        let count = u32::try_from(points.len()).map_err(|_| Error::from(E_FAIL))?;
        let mut index = 0u32;
        let mut exists = BOOL::default();
        // SAFETY: plain COM calls; the out pointers refer to live locals and `indices` holds one
        // slot per code point.
        unsafe {
            collection.FindFamilyName(
                &HSTRING::from(self.family_name(spec.family)),
                &raw mut index,
                &raw mut exists,
            )?;
            if !exists.as_bool() {
                return Err(Error::from(E_FAIL));
            }
            let face = collection
                .GetFontFamily(index)?
                .GetFirstMatchingFont(
                    DWRITE_FONT_WEIGHT(i32::from(spec.weight)),
                    DWRITE_FONT_STRETCH_NORMAL,
                    DWRITE_FONT_STYLE_NORMAL,
                )?
                .CreateFontFace()?;
            face.GetGlyphIndices(points.as_ptr(), count, indices.as_mut_ptr())?;
        }
        Ok(Some(
            shown
                .into_iter()
                .zip(indices)
                .filter(|(_, glyph)| *glyph == 0)
                .map(|(c, _)| c)
                .collect(),
        ))
    }
}
