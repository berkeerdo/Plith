//! Sets `PLITH_BUILD_DATE` (UTC, `YYYY-MM-DD`) for the About section and, for a Windows target,
//! embeds the Plith icon and a version resource (file and product version from Cargo's
//! `version`, so Explorer, the installer and an upgrade read the same number the About section
//! shows).

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let (y, m, d) = civil_from_days(i64::try_from(secs / 86_400).unwrap_or(0));
    println!("cargo:rustc-env=PLITH_BUILD_DATE={y:04}-{m:02}-{d:02}");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/plith.ico");
    embed_resources();
}

/// The icon (resource id 1, which the tray looks up) and the version strings.
fn embed_resources() {
    if std::env::var("CARGO_CFG_TARGET_OS").is_ok_and(|os| os != "windows") {
        return;
    }
    let dir = std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
    let icon = PathBuf::from(dir).join("assets").join("plith.ico");
    let mut res = winresource::WindowsResource::new();
    res.set_icon(&icon.display().to_string())
        .set("ProductName", "Plith")
        .set("FileDescription", "Plith")
        .set("OriginalFilename", "plith.exe")
        .set("InternalName", "plith");
    if let Err(error) = res.compile() {
        // A build without a resource compiler still runs; the tray then shows the stock icon.
        println!("cargo:warning=cannot embed the icon and version resource: {error}");
    }
}

/// Days since 1970-01-01 to a proleptic Gregorian (year, month, day).
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}
