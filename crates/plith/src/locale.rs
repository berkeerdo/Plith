//! The Windows language, read once at start for `lang=auto`.

use windows::Win32::Globalization::GetUserDefaultLocaleName;

/// The longest locale name, terminator included (`LOCALE_NAME_MAX_LENGTH`).
const NAME_MAX: usize = 85;

/// The user's locale name, for example `tr-TR`; empty when Windows gives none.
#[must_use]
pub fn windows_language() -> String {
    let mut name = [0u16; NAME_MAX];
    // SAFETY: the buffer is valid for its whole length, which the call receives.
    let written = unsafe { GetUserDefaultLocaleName(&mut name) };
    match usize::try_from(written) {
        Ok(len) if len > 1 => String::from_utf16_lossy(&name[..len - 1]),
        _ => String::new(),
    }
}
