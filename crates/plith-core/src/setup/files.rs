//! Which files under the install root belong to the new app.

/// True only for the three files Plith installs; everything else under the root is removed.
#[must_use]
pub fn is_ours(relative: &str) -> bool {
    let normal = relative.replace('/', "\\").to_ascii_lowercase();
    matches!(
        normal.as_str(),
        "plith.exe" | "osd\\plith-osd.exe" | "setup\\plith-setup.exe"
    )
}
