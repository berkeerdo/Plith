//! The shelf: a short list of file and folder references, newest first. Pure model.
//!
//! The shelf stages references. Nothing is copied, moved or deleted, and removing an item does
//! not touch the file. The text form is `shelf.txt`: one rooted path per line, newest first, the
//! format of the WPF shelf, so an existing shelf carries over.

use std::path::{Path, PathBuf};

pub mod hover;
pub mod kind;
pub mod layout;
pub mod line;
pub mod name;

/// How many items the shelf keeps: two rows of four wagons.
pub const CAPACITY: usize = 8;

/// One file or folder on the shelf.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShelfItem {
    /// The rooted path.
    pub path: PathBuf,
    /// True for a folder.
    pub dir: bool,
}

/// What an [`add`](Shelf::add) did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Added {
    /// Items that were not on the shelf before.
    pub new: usize,
    /// Items that were already there and moved to the front.
    pub moved: usize,
    /// Items pushed off the far end by the capacity.
    pub pushed_out: usize,
}

/// The shelf: items newest first, plus the last removal for one undo.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Shelf {
    items: Vec<ShelfItem>,
    /// The last removal (one item, or the whole shelf after a clear) with the old places, in
    /// ascending order.
    removed: Vec<(usize, ShelfItem)>,
}

/// The comparison key of a path: Windows paths ignore case and a trailing separator.
fn key(path: &Path) -> String {
    path.to_string_lossy()
        .trim_end_matches(['\\', '/'])
        .to_lowercase()
}

/// True when `path` starts at a root (a drive or a share), not relative.
#[must_use]
pub fn is_rooted(path: &Path) -> bool {
    path.is_absolute()
}

impl Shelf {
    /// The items, newest first.
    #[must_use]
    pub fn items(&self) -> &[ShelfItem] {
        &self.items
    }

    /// How many items are on the shelf.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// True when the shelf holds nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// True when a removal can be taken back.
    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.removed.is_empty()
    }

    fn position(&self, path: &Path) -> Option<usize> {
        let wanted = key(path);
        self.items.iter().position(|item| key(&item.path) == wanted)
    }

    /// Puts `batch` at the front, the first of the batch first. A path already on the shelf
    /// moves to the front instead of appearing twice; relative paths are ignored. Past
    /// [`CAPACITY`] the oldest items fall off.
    pub fn add(&mut self, batch: &[ShelfItem]) -> Added {
        let mut added = Added::default();
        let mut seen: Vec<String> = Vec::new();
        for item in batch.iter().rev() {
            let id = key(&item.path);
            if !is_rooted(&item.path) || seen.contains(&id) {
                continue;
            }
            seen.push(id);
            if let Some(at) = self.position(&item.path) {
                self.items.remove(at);
                added.moved += 1;
            } else {
                added.new += 1;
            }
            self.items.insert(0, item.clone());
        }
        let before = self.items.len();
        self.items.truncate(CAPACITY);
        added.pushed_out = before - self.items.len();
        added
    }

    /// Removes the item at `index` and remembers it for [`undo`](Self::undo). `false` when
    /// there is no such item.
    pub fn remove(&mut self, index: usize) -> bool {
        if index >= self.items.len() {
            return false;
        }
        let item = self.items.remove(index);
        self.removed = vec![(index, item)];
        true
    }

    /// Removes every item and remembers them all for one [`undo`](Self::undo). Returns how
    /// many were removed; an empty shelf keeps the earlier removal.
    pub fn clear(&mut self) -> usize {
        let count = self.items.len();
        if count > 0 {
            self.removed = self.items.drain(..).enumerate().collect();
        }
        count
    }

    /// Brings the last removal back at its old places. `false` when nothing was removed, or
    /// every path came back some other way meanwhile.
    pub fn undo(&mut self) -> bool {
        let mut restored = false;
        for (index, item) in std::mem::take(&mut self.removed) {
            if self.position(&item.path).is_some() {
                continue;
            }
            self.items.insert(index.min(self.items.len()), item);
            restored = true;
        }
        self.items.truncate(CAPACITY);
        restored
    }

    /// The text form: one path per line, newest first.
    #[must_use]
    pub fn serialize(&self) -> String {
        let mut out = String::new();
        for item in &self.items {
            // A name with an unpaired surrogate has no text form; the line is left out
            // rather than written corrupted.
            if let Some(text) = item.path.to_str() {
                out.push_str(text);
                out.push('\n');
            }
        }
        out
    }

    /// Reads the text form. `probe` says what became of a path: a line whose path is
    /// [`Probe::Gone`], a relative line, a blank line and a repeat are dropped; a path that
    /// cannot be told (an offline share) is kept. Only the first [`CAPACITY`] lines are kept.
    pub fn parse(text: &str, probe: impl Fn(&Path) -> Probe) -> Self {
        let mut shelf = Shelf::default();
        for line in text.trim_start_matches('\u{feff}').lines() {
            let path = PathBuf::from(line.trim());
            if line.trim().is_empty() || !is_rooted(&path) || shelf.position(&path).is_some() {
                continue;
            }
            let dir = match probe(&path) {
                Probe::Gone => continue,
                Probe::Dir => true,
                Probe::File | Probe::Unknown => false,
            };
            shelf.items.push(ShelfItem { path, dir });
            if shelf.items.len() == CAPACITY {
                break;
            }
        }
        shelf
    }

    /// Adds `older` behind the items already here (a shelf that loaded late): paths already
    /// present are skipped and the capacity holds.
    pub fn append_older(&mut self, older: &Shelf) {
        for item in &older.items {
            if self.items.len() == CAPACITY {
                break;
            }
            if self.position(&item.path).is_none() {
                self.items.push(item.clone());
            }
        }
    }
}

/// What a path probe found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Probe {
    /// A folder.
    Dir,
    /// A file.
    File,
    /// Definitely not there (the system said so).
    Gone,
    /// Could not be told (an offline share, no access): the entry stays.
    Unknown,
}

/// The first [`CAPACITY`] distinct rooted paths of `paths`, in order. A big drop is cut here,
/// before any per-path work, so a thousand files cost eight probes.
#[must_use]
pub fn newest(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut seen: Vec<String> = Vec::new();
    let mut out = Vec::new();
    for path in paths {
        if out.len() == CAPACITY {
            break;
        }
        let id = key(path);
        if is_rooted(path) && !seen.contains(&id) {
            seen.push(id);
            out.push(path.clone());
        }
    }
    out
}

#[cfg(test)]
mod tests;
