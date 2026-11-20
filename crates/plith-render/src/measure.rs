//! Text widths for the UI thread, such as the settings field's caret position. It never draws.

use crate::text::TextSystem;
use crate::thread::RenderError;
use plith_core::draw::{TextStyle, Trim};

/// A width no single line reaches, so nothing is trimmed.
const UNBOUNDED: f32 = 100_000.0;

/// Measures text with its own DirectWrite factory, fonts and formats: the same text system the
/// render threads draw with, so a width matches the drawn text. Use it on one thread.
pub struct TextMeasure {
    text: TextSystem,
}

impl TextMeasure {
    /// Creates the factory and loads the bundled fonts (system fonts when they cannot load).
    ///
    /// # Errors
    /// Returns [`RenderError::Init`] when DirectWrite cannot be created.
    pub fn new() -> Result<Self, RenderError> {
        Ok(Self {
            text: TextSystem::new().map_err(RenderError::Init)?,
        })
    }

    /// The width of `text` in `style` in DIP, trailing spaces included, as the renderer lays it
    /// out on one line (upper-cased for upper-case styles). Zero when DirectWrite fails.
    pub fn width(&mut self, text: &str, style: TextStyle) -> f64 {
        self.text
            .measure(text, style, UNBOUNDED, Trim::End)
            .map_or(0.0, f64::from)
    }
}
