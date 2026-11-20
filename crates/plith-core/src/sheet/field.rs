//! The state of the one text field (the city).

/// A one line text field.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextField {
    /// The text.
    pub text: String,
    /// The caret as a character index.
    pub caret: usize,
    /// Whether the whole text is selected.
    pub all: bool,
}
