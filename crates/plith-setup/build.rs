//! Prepares `plith-setup.exe`: stages the payload (the signed `plith.exe` and `plith-osd.exe`)
//! into `OUT_DIR` for `include_bytes!`, embeds the icon and version resource, and embeds the
//! manifest through link arguments.
//!
//! `PLITH_SETUP_PAYLOAD` names a folder holding the two signed exes. Unset (dev, clippy, tests)
//! both payload files are empty and the installer is not a release build.

use std::path::{Path, PathBuf};

/// The payload files, by name.
const PAYLOAD_FILES: [&str; 2] = ["plith.exe", "plith-osd.exe"];

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=plith-setup.manifest");
    println!("cargo:rerun-if-env-changed=PLITH_SETUP_PAYLOAD");
    stage_payload();
    if std::env::var("CARGO_CFG_TARGET_OS").is_ok_and(|os| os != "windows") {
        return;
    }
    embed_icon_and_version();
    embed_manifest();
}

/// Copies the payload folder's files to `OUT_DIR`, or writes empty stand-ins.
fn stage_payload() {
    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"));
    let source = std::env::var_os("PLITH_SETUP_PAYLOAD")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from);
    for name in PAYLOAD_FILES {
        let target = out.join(name);
        if let Some(folder) = &source {
            let from = folder.join(name);
            println!("cargo:rerun-if-changed={}", from.display());
            std::fs::copy(&from, &target)
                .unwrap_or_else(|error| panic!("cannot copy {}: {error}", from.display()));
        } else {
            std::fs::write(&target, []).expect("write an empty payload stand-in");
        }
    }
}

/// The icon is the app's own, referenced in place, never copied.
fn embed_icon_and_version() {
    let dir = std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
    let icon = Path::new(&dir).join("../plith/assets/plith.ico");
    println!("cargo:rerun-if-changed={}", icon.display());
    let mut res = winresource::WindowsResource::new();
    res.set_icon(&icon.display().to_string())
        .set("ProductName", "Plith")
        .set("FileDescription", "Plith Setup")
        .set("OriginalFilename", "plith-setup.exe")
        .set("InternalName", "plith-setup");
    if let Err(error) = res.compile() {
        // A build without a resource compiler still runs; Explorer then shows the stock icon.
        println!("cargo:warning=cannot embed the icon and version resource: {error}");
    }
}

/// The manifest (asInvoker, per-monitor DPI, long paths) goes in through the linker, so
/// winresource adds none and only one manifest is embedded.
fn embed_manifest() {
    if std::env::var("CARGO_CFG_TARGET_ENV").is_ok_and(|env| env != "msvc") {
        return;
    }
    let dir = std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
    let manifest = Path::new(&dir).join("plith-setup.manifest");
    // Every DLL this exe loads comes from System32 only: it may start elevated from Downloads,
    // where a planted wintrust.dll or crypt32.dll next to it would otherwise load first.
    println!("cargo:rustc-link-arg-bins=/DEPENDENTLOADFLAG:0x800");
    println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
    println!(
        "cargo:rustc-link-arg-bins=/MANIFESTINPUT:{}",
        manifest.display()
    );
    println!("cargo:rustc-link-arg-bins=/MANIFESTUAC:level='asInvoker' uiAccess='false'");
}
