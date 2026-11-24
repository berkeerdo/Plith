//! The type tag of a wagon, from the extension: `PNG`, `MP4`. A folder and a file with no
//! extension get a word in the interface language.

use super::ShelfItem;
use crate::lang::{Lang, Text, text};

/// The longest extension tag, in characters; a longer extension is cut.
pub const TAG_MAX: usize = 5;

/// The tag printed on a wagon: the extension in capitals, or the localized word for a folder
/// (`KLASÖR`) or for a file with no extension (`DOSYA`).
#[must_use]
pub fn type_tag(item: &ShelfItem, lang: Lang) -> String {
    if item.dir {
        return text(Text::ShelfFolder, lang).to_owned();
    }
    item.path
        .extension()
        .and_then(|ext| ext.to_str())
        .filter(|ext| !ext.is_empty())
        .map_or_else(
            || text(Text::ShelfFile, lang).to_owned(),
            |ext| ext.chars().take(TAG_MAX).collect::<String>().to_uppercase(),
        )
}

/// The full name of an item: its last path component, or the whole path for a drive root.
#[must_use]
pub fn display_name(item: &ShelfItem) -> String {
    item.path.file_name().map_or_else(
        || item.path.to_string_lossy().into_owned(),
        |name| name.to_string_lossy().into_owned(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn item(path: &str, dir: bool) -> ShelfItem {
        ShelfItem {
            path: PathBuf::from(path),
            dir,
        }
    }

    #[test]
    fn the_tag_is_the_extension_in_capitals() {
        assert_eq!(type_tag(&item("C:\\a\\Photo.png", false), Lang::Tr), "PNG");
        assert_eq!(type_tag(&item("C:\\a\\x.tar.gz", false), Lang::En), "GZ");
    }

    #[test]
    fn a_long_extension_is_cut() {
        assert_eq!(
            type_tag(&item("C:\\a\\x.torrent", false), Lang::Tr),
            "TORRE"
        );
    }

    #[test]
    fn a_folder_and_a_bare_file_get_the_word_of_the_language() {
        assert_eq!(type_tag(&item("C:\\a\\Photos", true), Lang::Tr), "KLASÖR");
        assert_eq!(type_tag(&item("C:\\a\\Makefile", false), Lang::Tr), "DOSYA");
        assert_eq!(
            type_tag(&item("C:\\a\\.gitignore", false), Lang::En),
            "FILE"
        );
        assert_eq!(type_tag(&item("C:\\a\\Photos", true), Lang::De), "ORDNER");
    }

    #[test]
    fn the_name_is_the_last_component() {
        assert_eq!(display_name(&item("C:\\a\\b\\c.txt", false)), "c.txt");
        assert_eq!(display_name(&item("C:\\", true)), "C:\\");
    }
}
