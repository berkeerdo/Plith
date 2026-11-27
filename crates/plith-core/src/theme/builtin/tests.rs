use super::*;
use crate::theme::pack::parse_pack;

/// Each manifest is the one in the folder named by its own id, so a pack cannot be included
/// from a neighbour's folder.
#[test]
fn builtin_ids_match_folders() {
    for pack in &BUILTIN {
        let id = parse_pack(pack.manifest).unwrap().id;
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("themes")
            .join(&id)
            .join("theme.ini");
        let on_disk = std::fs::read_to_string(path).unwrap();
        assert_eq!(pack.manifest, on_disk, "{id} is not its folder's manifest");
    }
}

#[test]
fn halka_files_present() {
    let files = BUILTIN[1].files;
    let names = [
        "floor-1x",
        "floor-2x",
        "ring-1-1x",
        "ring-1-2x",
        "ring-2-1x",
        "ring-2-2x",
        "ring-3-1x",
        "ring-3-2x",
        "disc-1x",
        "disc-2x",
    ];
    assert_eq!(files.len(), names.len());
    for name in names {
        let found = files.iter().find(|(n, _)| *n == name);
        assert!(found.is_some_and(|(_, bytes)| !bytes.is_empty()), "{name}");
    }
    assert!(BUILTIN[0].files.is_empty());
}
