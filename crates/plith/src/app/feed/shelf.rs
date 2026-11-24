//! The shelf on the feed: the items, the hot slot and the saver. Every change rebuilds the shelf
//! page only and saves the file; the page height follows the item count through the shapes.

use super::Feed;
use crate::pages::Dirty;
use plith_core::pages::PageId;
use plith_core::pages::shelf::ShelfView;
use plith_core::scene::SceneMailbox;
use plith_core::shelf::{Added, Shelf, ShelfItem};
use std::path::PathBuf;

/// Writes the shelf file.
pub type ShelfSaver = Box<dyn Fn(&Shelf)>;

/// What the feed keeps of the shelf.
#[derive(Default)]
pub struct ShelfState {
    shelf: Shelf,
    /// A file drag hovers over the island: the slot is drawn hot.
    hot: bool,
    saver: Option<ShelfSaver>,
    /// The saved shelf has been read. Until then nothing is written: a save of the few items
    /// dropped meanwhile would erase the file's lines.
    loaded: bool,
}

impl ShelfState {
    /// What the shelf page is built from.
    pub(super) fn view(&self) -> ShelfView<'_> {
        ShelfView {
            items: self.shelf.items(),
            hot: self.hot,
            can_undo: self.shelf.can_undo(),
        }
    }

    fn save(&self) {
        if let (Some(saver), true) = (&self.saver, self.loaded) {
            saver(&self.shelf);
        }
    }
}

impl Feed {
    /// Starts with an empty shelf and the way to save it, for a saved shelf that is still
    /// being read ([`Feed::shelf_loaded`] completes it). Nothing is saved until then.
    pub fn begin_shelf_load(&mut self, saver: ShelfSaver) {
        self.shelf = ShelfState {
            saver: Some(saver),
            ..ShelfState::default()
        };
    }

    /// The saved shelf arrived: it goes behind what was dropped meanwhile, then the file is
    /// written with both.
    pub fn shelf_loaded(&mut self, saved: &Shelf, mailbox: &SceneMailbox) {
        self.shelf.shelf.append_older(saved);
        self.shelf.loaded = true;
        self.shelf_changed(mailbox);
    }

    /// The items on the shelf, newest first.
    #[must_use]
    pub fn shelf_items(&self) -> &[ShelfItem] {
        self.shelf.shelf.items()
    }

    /// The path of the wagon at `index`, to drag out.
    #[must_use]
    pub fn shelf_path(&self, index: usize) -> Option<PathBuf> {
        self.shelf_items().get(index).map(|item| item.path.clone())
    }

    /// Puts `items` on the shelf, saves it and rebuilds the page.
    pub fn shelf_add(&mut self, items: &[ShelfItem], mailbox: &SceneMailbox) -> Added {
        let added = self.shelf.shelf.add(items);
        self.shelf_changed(mailbox);
        added
    }

    /// Takes the wagon at `index` off the shelf; the file is untouched.
    pub fn shelf_remove(&mut self, index: usize, mailbox: &SceneMailbox) -> bool {
        let done = self.shelf.shelf.remove(index);
        if done {
            self.shelf_changed(mailbox);
        }
        done
    }

    /// Brings the last removal back.
    pub fn shelf_undo(&mut self, mailbox: &SceneMailbox) -> bool {
        let done = self.shelf.shelf.undo();
        if done {
            self.shelf_changed(mailbox);
        }
        done
    }

    /// Draws the slot hot (or not) while a file drag hovers; rebuilds only on a change.
    pub fn set_shelf_hot(&mut self, hot: bool, mailbox: &SceneMailbox) {
        if self.shelf.hot != hot {
            self.shelf.hot = hot;
            self.rebuild(Dirty::page(PageId::Shelf), mailbox);
        }
    }

    fn shelf_changed(&mut self, mailbox: &SceneMailbox) {
        self.shelf.save();
        self.rebuild(Dirty::page(PageId::Shelf), mailbox);
    }
}
