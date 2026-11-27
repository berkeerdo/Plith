//! The built-in theme packs, one folder per pack under `crates/plith-core/themes/`: the manifest
//! text and the raster files beside it.

/// One built-in pack: its manifest and its raster files.
#[derive(Debug, Clone, Copy)]
pub struct BuiltinPack {
    /// The manifest text.
    pub manifest: &'static str,
    /// The PNG files by name, without extension (`ring-1-2x`).
    pub files: &'static [(&'static str, &'static [u8])],
}

/// Pairs a file name with its bytes: `file!("halka", "disc-1x")`.
macro_rules! file {
    ($pack:literal, $name:literal) => {
        (
            $name,
            include_bytes!(concat!("../../themes/", $pack, "/", $name, ".png")) as &[u8],
        )
    };
}

/// Every built-in pack. A built-in that refuses to parse is a test failure.
pub const BUILTIN: [BuiltinPack; 2] = [
    BuiltinPack {
        manifest: include_str!("../../themes/gece/theme.ini"),
        files: &[],
    },
    BuiltinPack {
        manifest: include_str!("../../themes/halka/theme.ini"),
        files: &[
            file!("halka", "floor-1x"),
            file!("halka", "floor-2x"),
            file!("halka", "ring-1-1x"),
            file!("halka", "ring-1-2x"),
            file!("halka", "ring-2-1x"),
            file!("halka", "ring-2-2x"),
            file!("halka", "ring-3-1x"),
            file!("halka", "ring-3-2x"),
            file!("halka", "disc-1x"),
            file!("halka", "disc-2x"),
        ],
    },
];

#[cfg(test)]
mod tests;
