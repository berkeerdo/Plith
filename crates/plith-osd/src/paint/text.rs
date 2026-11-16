//! DirectWrite text formats for the OSD: the family is picked from the installed fonts,
//! because DirectWrite silently substitutes a family it cannot find.

use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
    DWRITE_FONT_WEIGHT_MEDIUM, DWRITE_FONT_WEIGHT_REGULAR, DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
    DWRITE_TEXT_ALIGNMENT, DWRITE_TEXT_ALIGNMENT_LEADING, DWRITE_TEXT_ALIGNMENT_TRAILING,
    DWRITE_TRIMMING, DWRITE_TRIMMING_GRANULARITY_CHARACTER, DWRITE_WORD_WRAPPING_NO_WRAP,
    DWriteCreateFactory, IDWriteFactory, IDWriteFontCollection, IDWriteTextFormat,
};
use windows::core::{BOOL, HSTRING, Result, w};

/// Mono families in order of preference: the design font, then the one every Windows has.
pub const MONO_FAMILIES: [&str; 2] = ["Martian Mono", "Consolas"];

/// Label size in DIP.
const LABEL_DIP: f32 = 10.0;
/// Value size in DIP.
const VALUE_DIP: f32 = 13.0;

/// The three text formats the OSD draws with.
pub struct Fonts {
    /// The family every format uses.
    pub family: &'static str,
    /// Left slot: device name, 10 DIP, leading, ends in an ellipsis when too long.
    pub label: IDWriteTextFormat,
    /// Right slot: the number, 13 DIP, trailing.
    pub value: IDWriteTextFormat,
    /// Right slot when muted: the mute word, 10 DIP, trailing.
    pub mute: IDWriteTextFormat,
}

/// The first of `wanted` that `installed` reports, or the last one when none is.
#[must_use]
pub fn pick_family(wanted: &[&'static str], installed: impl Fn(&str) -> bool) -> &'static str {
    wanted
        .iter()
        .copied()
        .find(|name| installed(name))
        .or_else(|| wanted.last().copied())
        .unwrap_or("Consolas")
}

fn is_installed(collection: &IDWriteFontCollection, name: &str) -> bool {
    let (mut index, mut exists) = (0u32, BOOL(0));
    // SAFETY: the name is a live HSTRING and both out pointers are live locals.
    let found =
        unsafe { collection.FindFamilyName(&HSTRING::from(name), &raw mut index, &raw mut exists) };
    found.is_ok() && exists.as_bool()
}

fn format(
    factory: &IDWriteFactory,
    family: &str,
    size: f32,
    bold: bool,
    align: DWRITE_TEXT_ALIGNMENT,
) -> Result<IDWriteTextFormat> {
    let weight = if bold {
        DWRITE_FONT_WEIGHT_MEDIUM
    } else {
        DWRITE_FONT_WEIGHT_REGULAR
    };
    // SAFETY: plain COM calls; the strings are live for each call.
    unsafe {
        let format = factory.CreateTextFormat(
            &HSTRING::from(family),
            None,
            weight,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            size,
            w!("tr-TR"),
        )?;
        format.SetTextAlignment(align)?;
        format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
        format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
        Ok(format)
    }
}

/// Makes `format` trim at a character and end in an ellipsis instead of cutting a glyph.
fn ellipsized(factory: &IDWriteFactory, format: IDWriteTextFormat) -> Result<IDWriteTextFormat> {
    let trimming = DWRITE_TRIMMING {
        granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER,
        delimiter: 0,
        delimiterCount: 0,
    };
    // SAFETY: plain COM calls; `trimming` is a live local for the call.
    unsafe {
        let sign = factory.CreateEllipsisTrimmingSign(&format)?;
        format.SetTrimming(&raw const trimming, &sign)?;
    }
    Ok(format)
}

impl Fonts {
    /// Creates the shared DirectWrite factory and the three formats in the first installed
    /// mono family.
    ///
    /// # Errors
    /// Returns the first failing DirectWrite call.
    pub fn new() -> Result<Self> {
        // SAFETY: creating the shared factory has no preconditions.
        let factory: IDWriteFactory = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)? };
        let mut collection: Option<IDWriteFontCollection> = None;
        // SAFETY: `collection` is a live out pointer the call fills once.
        unsafe { factory.GetSystemFontCollection(&raw mut collection, false)? };
        let family = match collection {
            Some(collection) => pick_family(&MONO_FAMILIES, |name| is_installed(&collection, name)),
            None => pick_family(&MONO_FAMILIES, |_| false),
        };
        Ok(Self {
            family,
            label: ellipsized(
                &factory,
                format(
                    &factory,
                    family,
                    LABEL_DIP,
                    false,
                    DWRITE_TEXT_ALIGNMENT_LEADING,
                )?,
            )?,
            value: format(
                &factory,
                family,
                VALUE_DIP,
                true,
                DWRITE_TEXT_ALIGNMENT_TRAILING,
            )?,
            mute: format(
                &factory,
                family,
                LABEL_DIP,
                false,
                DWRITE_TEXT_ALIGNMENT_TRAILING,
            )?,
        })
    }
}
