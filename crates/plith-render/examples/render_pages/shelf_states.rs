//! The shelf page with 0, 1, 4, 5 and 8 wagons and the hot slot, for the R5 review against the
//! mock (`shelfPage()` and `shelfEmpty()` in `docs/design/r3/ray-paneli.html`).

use super::Content;
use plith_core::lang::Lang;
use plith_core::pages::PageId;
use plith_core::pages::fixtures;
use plith_core::pages::shelf::{ShelfView, shelf_page_with};
use plith_core::shelf::ShelfItem;
use std::path::PathBuf;

/// Names that cover a short name, the mock's four, a long one with a separator, one with none
/// and a folder.
const NAMES: [(&str, bool); 8] = [
    ("assets final.png", false),
    ("2026-11-14.mp4", false),
    ("Natro.pdf", false),
    ("ayarlar.zip", false),
    ("Voicemeeter Banana kurulum notlari.docx", false),
    ("IMG_20261114_231547_burst0001.jpg", false),
    ("Projeler", true),
    ("a.txt", false),
];

fn items(count: usize) -> Vec<ShelfItem> {
    NAMES
        .iter()
        .take(count)
        .map(|(name, dir)| ShelfItem {
            path: PathBuf::from(format!("C:\\Users\\Berke\\Desktop\\{name}")),
            dir: *dir,
        })
        .collect()
}

fn page(count: usize, hot: bool, can_undo: bool, lang: Lang) -> Vec<Content> {
    let items = items(count);
    let view = ShelfView {
        items: &items,
        hot,
        can_undo,
    };
    vec![Content::Page(
        PageId::Shelf,
        shelf_page_with(&view, fixtures::CLOCK, lang),
    )]
}

/// The states to render in `lang`, by file name stem.
pub fn states(lang: Lang) -> Vec<(&'static str, Vec<Content>)> {
    vec![
        ("shelf-0-empty", page(0, false, false, lang)),
        ("shelf-0-hot", page(0, true, false, lang)),
        ("shelf-0-undo", page(0, false, true, lang)),
        ("shelf-1", page(1, false, false, lang)),
        ("shelf-3-hot", page(3, true, false, lang)),
        ("shelf-4", page(4, false, false, lang)),
        ("shelf-4-hot", page(4, true, true, lang)),
        ("shelf-5", page(5, false, false, lang)),
        ("shelf-8", page(8, false, true, lang)),
        ("shelf-8-hot", page(8, true, false, lang)),
    ]
}
