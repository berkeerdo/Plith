//! Middle trimming: keep the head and the tail of a line and put an ellipsis between them.

use windows::core::Result;

/// The ellipsis character inserted by a middle trim (U+2026).
pub const ELLIPSIS: char = '\u{2026}';

/// Joins the first `head` and the last `tail` characters of `chars` around the ellipsis.
fn join(chars: &[char], head: usize, tail: usize) -> String {
    let mut out: String = chars[..head].iter().collect();
    out.push(ELLIPSIS);
    out.extend(&chars[chars.len() - tail..]);
    out
}

/// Returns `text` when `fits` accepts it, otherwise the longest `head + ELLIPSIS + tail` form
/// that `fits` accepts. Kept characters split evenly, the head gets the extra one. The search is
/// binary over the number of kept characters and works on `char` boundaries. When even a bare
/// ellipsis does not fit, the bare ellipsis is returned.
///
/// # Errors
/// Returns the first error `fits` reports.
pub fn middle_trim(text: &str, mut fits: impl FnMut(&str) -> Result<bool>) -> Result<String> {
    if fits(text)? {
        return Ok(text.to_owned());
    }
    let chars: Vec<char> = text.chars().collect();
    let candidate = |kept: usize| join(&chars, kept.div_ceil(2), kept / 2);
    // Invariant: `lo` kept characters fit (or is zero), `hi` kept characters do not.
    let (mut lo, mut hi) = (0, chars.len());
    while hi - lo > 1 {
        let mid = lo + (hi - lo) / 2;
        if fits(&candidate(mid))? {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Ok(candidate(lo))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn by_len(limit: usize) -> impl FnMut(&str) -> Result<bool> {
        move |s| Ok(s.chars().count() <= limit)
    }

    #[test]
    fn short_text_is_unchanged() {
        assert_eq!(middle_trim("abc", by_len(3)).unwrap(), "abc");
    }

    #[test]
    fn head_gets_the_extra_character() {
        // limit 6 = 5 kept + the ellipsis: head 3, tail 2.
        assert_eq!(
            middle_trim("abcdefghij", by_len(6)).unwrap(),
            "abc\u{2026}ij"
        );
    }

    #[test]
    fn works_on_char_boundaries() {
        assert_eq!(
            middle_trim("\u{15f}\u{15f}\u{15f}\u{15f}\u{15f}\u{15f}", by_len(3)).unwrap(),
            "\u{15f}\u{2026}\u{15f}"
        );
    }

    #[test]
    fn nothing_fits_gives_a_bare_ellipsis() {
        assert_eq!(middle_trim("abcdef", by_len(0)).unwrap(), "\u{2026}");
    }
}
