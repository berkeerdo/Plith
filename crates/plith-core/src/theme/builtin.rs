//! The built-in theme packs, one folder per pack under `crates/plith-core/themes/`. Plan 2 adds
//! the rasters beside each manifest.

/// The manifest text of every built-in pack. A built-in that refuses to parse is a test failure.
pub const BUILTIN: [&str; 1] = [include_str!("../../themes/gece/theme.ini")];
