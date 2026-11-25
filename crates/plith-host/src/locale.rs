//! The Windows display language, read once at start for `lang=auto`.
//!
//! This is the UI language (the one the Settings app calls "Windows display language"), not the
//! regional format: an English Windows with a Turkish region must read as English.

use windows::Win32::Globalization::{
    GetUserDefaultUILanguage, GetUserPreferredUILanguages, LCIDToLocaleName, MUI_LANGUAGE_NAME,
};

/// The longest locale name, terminator included (`LOCALE_NAME_MAX_LENGTH`).
const NAME_MAX: usize = 85;

/// Room for the preferred-language list: a few names of at most `NAME_MAX` code units.
const LIST_MAX: usize = 512;

/// The user's display language name, for example `tr-TR`; empty when Windows gives none.
#[must_use]
pub fn windows_language() -> String {
    preferred_ui_language()
        .or_else(default_ui_language)
        .unwrap_or_default()
}

/// The first entry of the user's preferred UI languages, the one Windows shows itself in.
fn preferred_ui_language() -> Option<String> {
    let mut count = 0u32;
    let mut list = [0u16; LIST_MAX];
    let mut length = u32::try_from(list.len()).ok()?;
    // SAFETY: `list` is valid for `length` code units, which the call receives and updates; the
    // count pointer is valid for one `u32`.
    unsafe {
        GetUserPreferredUILanguages(
            MUI_LANGUAGE_NAME,
            &raw mut count,
            Some(windows::core::PWSTR(list.as_mut_ptr())),
            &raw mut length,
        )
    }
    .ok()?;
    first_name(&list)
}

/// The UI language of the user's default LCID, as a locale name.
fn default_ui_language() -> Option<String> {
    let mut name = [0u16; NAME_MAX];
    // SAFETY: both calls have no preconditions; the buffer is valid for its whole length, which
    // the second call receives.
    let written = unsafe {
        let lcid = u32::from(GetUserDefaultUILanguage());
        LCIDToLocaleName(lcid, Some(&mut name), 0)
    };
    let len = usize::try_from(written).ok().filter(|&len| len > 1)?;
    Some(String::from_utf16_lossy(&name[..len - 1]))
}

/// The first name of a multi-string (names separated by NUL, ended by an empty one).
fn first_name(list: &[u16]) -> Option<String> {
    let end = list.iter().position(|&unit| unit == 0)?;
    (end > 0).then(|| String::from_utf16_lossy(&list[..end]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().collect()
    }

    #[test]
    fn the_first_entry_of_the_list_is_taken() {
        let mut list = wide("en-US");
        list.push(0);
        list.extend(wide("tr-TR"));
        list.extend([0, 0]);
        assert_eq!(first_name(&list).as_deref(), Some("en-US"));
    }

    #[test]
    fn an_empty_list_has_no_first_entry() {
        assert_eq!(first_name(&[0, 0]), None);
        assert_eq!(first_name(&[]), None);
    }

    #[test]
    fn this_machine_answers_with_a_language_name() {
        // Not asserted to be a particular language: only that the call works and is well formed.
        let name = windows_language();
        assert!(
            name.is_empty() || name.contains('-') || name.len() >= 2,
            "{name}"
        );
    }
}
