//! Embeds the application manifest into `plith-osd.exe`.
//!
//! `PLITH_OSD_UIACCESS=1` at build time requests `uiAccess='true'`; otherwise the
//! manifest says `'false'` so dev runs and `cargo test` work from any folder.

use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=PLITH_OSD_UIACCESS");
    println!("cargo:rerun-if-changed=plith-osd.manifest");

    let is_msvc = std::env::var("CARGO_CFG_TARGET_ENV").is_ok_and(|env| env == "msvc");
    if !is_msvc {
        return;
    }

    let dir = std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
    let manifest = PathBuf::from(dir).join("plith-osd.manifest");
    let ui_access = std::env::var("PLITH_OSD_UIACCESS").is_ok_and(|v| v == "1");

    println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
    println!(
        "cargo:rustc-link-arg-bins=/MANIFESTINPUT:{}",
        manifest.display()
    );
    println!("cargo:rustc-link-arg-bins=/MANIFESTUAC:level='asInvoker' uiAccess='{ui_access}'");
}
