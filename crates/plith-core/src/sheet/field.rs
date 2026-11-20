//! The state and editing of the one text field (the city).

use super::keys::SheetKey;

/// The most characters the field holds.
pub const FIELD_MAX: usize = 64;

/// A one line text field.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextField {
    /// The text.
    pub text: String,
    /// The caret as a character index.
    pub caret: usize,
    /// Whether the whole text is selected.
    pub all: bool,
    /// The saved value the field was opened with; a commit is only news when the text differs.
    pub saved: String,
}

/// What a key did to the field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldOutcome {
    /// The text changed.
    Edited,
    /// Only the caret or the selection moved.
    Moved,
    /// Enter: the text to commit.
    Commit(String),
    /// Escape: the caller restores the saved text and leaves the field.
    Cancel,
    /// Copy: the text for the clipboard.
    Copy(String),
    /// The key does nothing here.
    Ignored,
}

impl TextField {
    /// Loads `text` with the caret at its end and remembers it as the saved value.
    pub fn load(&mut self, text: &str) {
        self.text = text.chars().take(FIELD_MAX).collect();
        self.caret = self.len();
        self.all = false;
        self.saved.clone_from(&self.text);
    }

    fn len(&self) -> usize {
        self.text.chars().count()
    }

    fn byte(&self, chars: usize) -> usize {
        self.text
            .char_indices()
            .nth(chars)
            .map_or(self.text.len(), |(i, _)| i)
    }

    /// Replaces the selection (if any) with `s`, cut to the room left.
    fn insert(&mut self, s: &str) -> FieldOutcome {
        let room = if self.all {
            FIELD_MAX
        } else {
            FIELD_MAX.saturating_sub(self.len())
        };
        let add: String = s.chars().filter(|c| !c.is_control()).take(room).collect();
        if add.is_empty() {
            return FieldOutcome::Ignored;
        }
        if self.all {
            self.text.clear();
            self.caret = 0;
            self.all = false;
        }
        let at = self.byte(self.caret);
        self.text.insert_str(at, &add);
        self.caret += add.chars().count();
        FieldOutcome::Edited
    }

    fn remove(&mut self, back: bool) -> FieldOutcome {
        if self.all {
            self.text.clear();
            self.caret = 0;
            self.all = false;
            return FieldOutcome::Edited;
        }
        let at = if back {
            match self.caret.checked_sub(1) {
                Some(at) => at,
                None => return FieldOutcome::Ignored,
            }
        } else if self.caret < self.len() {
            self.caret
        } else {
            return FieldOutcome::Ignored;
        };
        let start = self.byte(at);
        self.text.remove(start);
        self.caret = at;
        FieldOutcome::Edited
    }

    fn go(&mut self, to: usize) -> FieldOutcome {
        let was = (self.caret, self.all);
        self.caret = to.min(self.len());
        self.all = false;
        if was == (self.caret, false) {
            FieldOutcome::Ignored
        } else {
            FieldOutcome::Moved
        }
    }

    /// Applies one key.
    pub fn edit(&mut self, key: &SheetKey) -> FieldOutcome {
        match key {
            SheetKey::Char(c) => self.insert(c.encode_utf8(&mut [0; 4])),
            SheetKey::Space => self.insert(" "),
            SheetKey::Paste(s) => self.insert(s),
            SheetKey::Backspace => self.remove(true),
            SheetKey::Delete => self.remove(false),
            SheetKey::Left if self.all => self.go(0),
            SheetKey::Left => self.go(self.caret.saturating_sub(1)),
            SheetKey::Right if self.all => self.go(self.len()),
            SheetKey::Right => self.go(self.caret + 1),
            SheetKey::Home => self.go(0),
            SheetKey::End => self.go(self.len()),
            SheetKey::SelectAll => {
                self.all = true;
                self.caret = self.len();
                FieldOutcome::Moved
            }
            SheetKey::Copy if self.all && !self.text.is_empty() => {
                FieldOutcome::Copy(self.text.clone())
            }
            SheetKey::Enter => FieldOutcome::Commit(self.text.clone()),
            SheetKey::Escape => FieldOutcome::Cancel,
            _ => FieldOutcome::Ignored,
        }
    }
}

#[cfg(test)]
mod tests;
