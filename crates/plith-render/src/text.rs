//! Text with DirectWrite: the bundled fonts, text formats, cached layouts, tabular numerals and
//! middle trimming. Render thread only; no Direct2D object lives here.

mod trim;

use std::collections::HashMap;

use plith_core::draw::{Family, TextStyle, Trim, style_spec};
use windows::Win32::Foundation::E_FAIL;
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FACTORY_TYPE_ISOLATED, DWRITE_FONT_FAMILY_MODEL_TYPOGRAPHIC, DWRITE_FONT_FEATURE,
    DWRITE_FONT_FEATURE_TAG_TABULAR_FIGURES, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
    DWRITE_FONT_WEIGHT, DWRITE_TEXT_METRICS, DWRITE_TEXT_RANGE, DWRITE_TRIMMING,
    DWRITE_TRIMMING_GRANULARITY_CHARACTER, DWRITE_WORD_WRAPPING_NO_WRAP, DWriteCreateFactory,
    IDWriteFactory5, IDWriteFactory6, IDWriteFontCollection, IDWriteFontCollection1,
    IDWriteInMemoryFontFileLoader, IDWriteInlineObject, IDWriteTextFormat, IDWriteTextLayout,
    IDWriteTextLayout1, IDWriteTypography,
};
use windows::core::{BOOL, Error, HSTRING, IUnknown, Interface, Result, w};

pub use trim::{ELLIPSIS, middle_trim};

/// The most layouts the cache keeps before it is cleared.
const CACHE_CAP: usize = 512;
/// A width no single line reaches; used to measure a line without wrapping.
const UNBOUNDED: f32 = 100_000.0;
/// Layout height; lines never wrap, so only the first line matters.
const LAYOUT_H: f32 = 1_000.0;

/// The sans family name DirectWrite reports for the bundled files.
const SANS_BUNDLED: &str = "Public Sans";
/// The mono family name DirectWrite reports for the bundled files.
const MONO_BUNDLED: &str = "Martian Mono";

static PUBLIC_SANS_REGULAR: &[u8] = include_bytes!("../assets/fonts/PublicSans-Regular.ttf");
static PUBLIC_SANS_SEMIBOLD: &[u8] = include_bytes!("../assets/fonts/PublicSans-SemiBold.ttf");
static MARTIAN_MONO_REGULAR: &[u8] = include_bytes!("../assets/fonts/MartianMono-StdRg.ttf");
static MARTIAN_MONO_MEDIUM: &[u8] = include_bytes!("../assets/fonts/MartianMono-StdMd.ttf");

type Key = (String, TextStyle, u32, Trim);

/// The font choice: an optional private collection and the two family names.
struct Fonts {
    collection: Option<IDWriteFontCollection>,
    sans: String,
    mono: String,
}

/// Fonts, formats and cached layouts. Created and used on the render thread only.
pub struct TextSystem {
    factory: IDWriteFactory5,
    loader: Option<IDWriteInMemoryFontFileLoader>,
    fonts: Fonts,
    typography: IDWriteTypography,
    formats: HashMap<(TextStyle, Trim), (IDWriteTextFormat, Option<IDWriteInlineObject>)>,
    cache: HashMap<Key, IDWriteTextLayout>,
    hits: u64,
    notes: Vec<String>,
}

fn has_family(collection: &IDWriteFontCollection, name: &str) -> bool {
    let mut index = 0u32;
    let mut exists = BOOL::default();
    // SAFETY: both out pointers refer to live locals and the name is a valid HSTRING.
    let found =
        unsafe { collection.FindFamilyName(&HSTRING::from(name), &raw mut index, &raw mut exists) };
    found.is_ok() && exists.as_bool()
}

/// Loads the four bundled faces into a private collection and returns it with the loader that
/// must stay registered while the collection is in use.
fn bundled(
    factory: &IDWriteFactory5,
) -> Result<(IDWriteInMemoryFontFileLoader, IDWriteFontCollection)> {
    // SAFETY: plain COM calls on live interfaces; the font data is `'static`, so the loader
    // never reads freed memory and needs no owner object.
    unsafe {
        let loader = factory.CreateInMemoryFontFileLoader()?;
        factory.RegisterFontFileLoader(&loader)?;
        let built = (|| {
            let builder = factory.CreateFontSetBuilder()?;
            for data in [
                PUBLIC_SANS_REGULAR,
                PUBLIC_SANS_SEMIBOLD,
                MARTIAN_MONO_REGULAR,
                MARTIAN_MONO_MEDIUM,
            ] {
                let size = u32::try_from(data.len()).map_err(|_| Error::from(E_FAIL))?;
                let file = loader.CreateInMemoryFontFileReference(
                    factory,
                    data.as_ptr().cast(),
                    size,
                    None::<&IUnknown>,
                )?;
                builder.AddFontFile(&file)?;
            }
            let set = builder.CreateFontSet()?;
            // The typographic model groups faces by name ID 16, so Regular and Medium share
            // one family; the default model would split them into "... Std Rg" and "... Std Md".
            let collection = factory
                .cast::<IDWriteFactory6>()?
                .CreateFontCollectionFromFontSet(&set, DWRITE_FONT_FAMILY_MODEL_TYPOGRAPHIC)?
                .cast::<IDWriteFontCollection>()?;
            // DirectWrite accepts a missing family name silently, so check the names here.
            if has_family(&collection, SANS_BUNDLED) && has_family(&collection, MONO_BUNDLED) {
                Ok(collection)
            } else {
                Err(Error::from(E_FAIL))
            }
        })();
        match built {
            Ok(collection) => Ok((loader, collection)),
            Err(error) => {
                let _ = factory.UnregisterFontFileLoader(&loader);
                Err(error)
            }
        }
    }
}

/// Picks the first installed family of `candidates` from the system collection; the last
/// candidate when none is found.
fn system_family(factory: &IDWriteFactory5, candidates: &[&str]) -> String {
    let mut system: Option<IDWriteFontCollection1> = None;
    // SAFETY: the out pointer refers to a live local.
    let ok = unsafe { factory.GetSystemFontCollection(false, &raw mut system, false) }.is_ok();
    if let (true, Some(system)) = (ok, system) {
        for name in candidates {
            if has_family(&system, name) {
                return (*name).to_owned();
            }
        }
    }
    candidates[candidates.len() - 1].to_owned()
}

impl TextSystem {
    /// Creates the factory and the bundled font collection; when that cannot be built the
    /// system fonts (Segoe UI Variable, Cascadia Mono or Consolas) are used and a note says so.
    ///
    /// # Errors
    /// Returns the failing DirectWrite call when even the factory cannot be created.
    pub fn new() -> Result<Self> {
        Self::build(true)
    }

    /// Like [`TextSystem::new`] but skips the bundled fonts, as when their set cannot be built.
    ///
    /// # Errors
    /// Returns the failing DirectWrite call when the factory cannot be created.
    #[doc(hidden)]
    pub fn new_system_fonts() -> Result<Self> {
        Self::build(false)
    }

    fn build(try_bundled: bool) -> Result<Self> {
        // ISOLATED: the in-memory loader is registered on this factory only, so no other
        // DirectWrite user in the process sees it or its cache, and it is unregistered on drop.
        // SAFETY: plain COM factory creation.
        let factory: IDWriteFactory5 =
            unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_ISOLATED)? };
        let mut notes = Vec::new();
        let attempt = if try_bundled {
            match bundled(&factory) {
                Ok(pair) => Some(pair),
                Err(error) => {
                    notes.push(format!(
                        "bundled fonts unavailable ({error}), using system fonts"
                    ));
                    None
                }
            }
        } else {
            notes.push("bundled fonts skipped, using system fonts".to_owned());
            None
        };
        let (loader, fonts) = match attempt {
            Some((loader, collection)) => (
                Some(loader),
                Fonts {
                    collection: Some(collection),
                    sans: SANS_BUNDLED.to_owned(),
                    mono: MONO_BUNDLED.to_owned(),
                },
            ),
            None => (
                None,
                Fonts {
                    collection: None,
                    sans: system_family(&factory, &["Segoe UI Variable Text", "Segoe UI"]),
                    mono: system_family(&factory, &["Cascadia Mono", "Consolas"]),
                },
            ),
        };
        // SAFETY: plain COM calls; the feature is a value struct.
        let typography = unsafe {
            let typography = factory.CreateTypography()?;
            typography.AddFontFeature(DWRITE_FONT_FEATURE {
                nameTag: DWRITE_FONT_FEATURE_TAG_TABULAR_FIGURES,
                parameter: 1,
            })?;
            typography
        };
        Ok(Self {
            factory,
            loader,
            fonts,
            typography,
            formats: HashMap::new(),
            cache: HashMap::new(),
            hits: 0,
            notes,
        })
    }

    /// Drains the notes recorded so far (one per font fallback), for the log.
    pub fn take_notes(&mut self) -> Vec<String> {
        std::mem::take(&mut self.notes)
    }

    /// True when the bundled fonts are in use.
    #[must_use]
    pub fn uses_bundled_fonts(&self) -> bool {
        self.fonts.collection.is_some()
    }

    /// The family name used for `family`.
    #[must_use]
    pub fn family_name(&self, family: Family) -> &str {
        match family {
            Family::Sans => &self.fonts.sans,
            Family::Mono => &self.fonts.mono,
        }
    }

    /// Test hook: the weight of the bundled face the collection matches for `style`'s family
    /// and requested weight, or `None` when the system fonts are in use. A missing face shows
    /// up as a different weight than requested.
    ///
    /// # Errors
    /// Returns the failing DirectWrite call.
    #[doc(hidden)]
    pub fn matched_weight(&self, style: TextStyle) -> Result<Option<u16>> {
        let Some(collection) = &self.fonts.collection else {
            return Ok(None);
        };
        let spec = style_spec(style);
        let mut index = 0u32;
        let mut exists = BOOL::default();
        // SAFETY: plain COM calls; the out pointers refer to live locals.
        let weight = unsafe {
            collection.FindFamilyName(
                &HSTRING::from(self.family_name(spec.family)),
                &raw mut index,
                &raw mut exists,
            )?;
            if !exists.as_bool() {
                return Err(Error::from(E_FAIL));
            }
            collection
                .GetFontFamily(index)?
                .GetFirstMatchingFont(
                    DWRITE_FONT_WEIGHT(i32::from(spec.weight)),
                    DWRITE_FONT_STRETCH_NORMAL,
                    DWRITE_FONT_STYLE_NORMAL,
                )?
                .GetWeight()
        };
        Ok(u16::try_from(weight.0).ok())
    }

    /// How many layouts the cache holds.
    #[must_use]
    #[doc(hidden)]
    pub fn cache_len(&self) -> usize {
        self.cache.len()
    }

    /// How many `layout` calls were answered from the cache.
    #[must_use]
    #[doc(hidden)]
    pub fn cache_hits(&self) -> u64 {
        self.hits
    }

    fn format(&mut self, style: TextStyle, trim: Trim) -> Result<IDWriteTextFormat> {
        if let Some((format, _)) = self.formats.get(&(style, trim)) {
            return Ok(format.clone());
        }
        let spec = style_spec(style);
        let name = HSTRING::from(self.family_name(spec.family));
        let weight = DWRITE_FONT_WEIGHT(i32::from(spec.weight));
        // SAFETY: plain COM calls on live interfaces; the optional collection is a live field.
        let (format, sign) = unsafe {
            let format = self.factory.CreateTextFormat(
                &name,
                self.fonts.collection.as_ref(),
                weight,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                spec.size_dip,
                w!("en-us"),
            )?;
            format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
            let sign = if trim == Trim::End {
                let sign = self.factory.CreateEllipsisTrimmingSign(&format)?;
                let options = DWRITE_TRIMMING {
                    granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER,
                    delimiter: 0,
                    delimiterCount: 0,
                };
                format.SetTrimming(&raw const options, &sign)?;
                Some(sign)
            } else {
                None
            };
            (format, sign)
        };
        self.formats.insert((style, trim), (format.clone(), sign));
        Ok(format)
    }

    /// Builds an uncached layout of already prepared text.
    fn build_layout(
        &mut self,
        text: &str,
        style: TextStyle,
        max_w: f32,
        trim: Trim,
    ) -> Result<IDWriteTextLayout> {
        let spec = style_spec(style);
        let format = self.format(style, trim)?;
        let wide: Vec<u16> = text.encode_utf16().collect();
        let range = DWRITE_TEXT_RANGE {
            startPosition: 0,
            length: u32::try_from(wide.len()).map_err(|_| Error::from(E_FAIL))?,
        };
        // SAFETY: plain COM calls on live interfaces; `wide` outlives the CreateTextLayout call.
        unsafe {
            let layout = self
                .factory
                .CreateTextLayout(&wide, &format, max_w, LAYOUT_H)?;
            if spec.family == Family::Mono {
                // Martian Mono has no `tnum` feature (its digits are already tabular), so this
                // call matters for the fallback fonts (Cascadia Mono, Consolas).
                layout.SetTypography(&self.typography, range)?;
            }
            if spec.tracking_em > 0.0 {
                let layout1: IDWriteTextLayout1 = layout.cast()?;
                layout1.SetCharacterSpacing(0.0, spec.tracking_em * spec.size_dip, 0.0, range)?;
            }
            Ok(layout)
        }
    }

    /// The text as it will be drawn: upper-cased for upper-case styles, and middle-trimmed to
    /// `max_w` for [`Trim::Middle`]. End trimming is left to DirectWrite, so the text is returned
    /// whole for it.
    ///
    /// # Errors
    /// Returns the failing DirectWrite call.
    pub fn display_text(
        &mut self,
        text: &str,
        style: TextStyle,
        max_w: f32,
        trim: Trim,
    ) -> Result<String> {
        let prepared = if style_spec(style).uppercase {
            // `to_uppercase` maps `i` to `I`, not the Turkish dotted capital; labels are already
            // written in upper case in the Turkish copy, so this only guards lower-case input.
            text.to_uppercase()
        } else {
            text.to_owned()
        };
        if trim == Trim::End {
            return Ok(prepared);
        }
        middle_trim(&prepared, |candidate| {
            Ok(self.line_width(candidate, style)? <= max_w)
        })
    }

    fn line_width(&mut self, text: &str, style: TextStyle) -> Result<f32> {
        let layout = self.build_layout(text, style, UNBOUNDED, Trim::Middle)?;
        let mut metrics = DWRITE_TEXT_METRICS::default();
        // SAFETY: the out pointer refers to a live local.
        unsafe { layout.GetMetrics(&raw mut metrics)? };
        Ok(metrics.widthIncludingTrailingWhitespace)
    }

    /// The width of `text` in DIP as `layout` would lay it out (after upper-casing and trimming).
    ///
    /// # Errors
    /// Returns the failing DirectWrite call.
    pub fn measure(&mut self, text: &str, style: TextStyle, max_w: f32, trim: Trim) -> Result<f32> {
        let shown = self.display_text(text, style, max_w, trim)?;
        self.line_width(&shown, style)
    }

    /// The returned layout must not outlive this `TextSystem`: its font loader is unregistered
    /// on drop.
    ///
    /// A layout of `text` for `style`, at most `max_w` DIP wide on one line. Cached by
    /// `(text, style, max_w, trim)`; the cache holds 512 entries and is cleared when full.
    ///
    /// # Errors
    /// Returns the failing DirectWrite call.
    pub fn layout(
        &mut self,
        text: &str,
        style: TextStyle,
        max_w: f32,
        trim: Trim,
    ) -> Result<IDWriteTextLayout> {
        let key = (text.to_owned(), style, max_w.to_bits(), trim);
        if let Some(layout) = self.cache.get(&key) {
            self.hits += 1;
            return Ok(layout.clone());
        }
        let shown = self.display_text(text, style, max_w, trim)?;
        let layout = self.build_layout(&shown, style, max_w, trim)?;
        if self.cache.len() >= CACHE_CAP {
            self.cache.clear();
        }
        self.cache.insert(key, layout.clone());
        Ok(layout)
    }
}

impl Drop for TextSystem {
    fn drop(&mut self) {
        if let Some(loader) = &self.loader {
            // SAFETY: the loader was registered on this factory in `bundled`.
            let _ = unsafe { self.factory.UnregisterFontFileLoader(loader) };
        }
    }
}
