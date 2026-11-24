//! The text of the HKCU Run value `Plith` and what a value found there means.
//!
//! The WPF app wrote the same value name with the quoted full path, so a value of either app is
//! recognised and a switch from this app takes it over.

/// The Run key under `HKEY_CURRENT_USER`.
pub const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

/// The value name, the one the WPF app used.
pub const RUN_VALUE: &str = "Plith";

/// The file name of the app, compared without case.
const EXE_NAME: &str = "plith.exe";

/// The value that starts the app at sign in: the full path in quotes, so a path with spaces
/// is read as one program.
#[must_use]
pub fn run_value(exe: &str) -> String {
    format!("\"{exe}\"")
}

/// The program path a Run value starts: the text inside the first pair of quotes, else the text
/// up to and including the first `.exe` that ends the value or is followed by a space, else the
/// whole value. Arguments are dropped.
#[must_use]
pub fn program_of(value: &str) -> &str {
    let value = value.trim();
    if let Some(rest) = value.strip_prefix('"') {
        return rest.split('"').next().unwrap_or("");
    }
    let lower = value.to_ascii_lowercase();
    let mut from = 0;
    while let Some(found) = lower[from..].find(".exe") {
        let end = from + found + ".exe".len();
        if lower[end..].is_empty() || lower[end..].starts_with(' ') {
            return &value[..end];
        }
        from = end;
    }
    value
}

/// Whether a Run value starts a Plith: its program is a file called `plith.exe`, in any folder
/// and in any case (the WPF app's was `Plith.exe`). A value of another program under the name
/// `Plith` is not ours: the row shows off and a switch on replaces it.
#[must_use]
pub fn is_ours(value: &str) -> bool {
    let program = program_of(value);
    let name = program.rsplit(['\\', '/']).next().unwrap_or("");
    name.eq_ignore_ascii_case(EXE_NAME)
}

#[cfg(test)]
mod tests;
