//! A file name on two lines. The painter cannot wrap, so the split is made here: the first line
//! breaks after a separator when one is near, else at the limit. The second line takes the rest
//! and the painter cuts it in the middle when it is still too long, which keeps the extension.

/// How many characters the first line holds (a wagon text box is 84 DIP of 12 px text).
pub const LINE_CHARS: usize = 12;
/// The earliest character a break after a separator may sit at.
const MIN_BREAK: usize = 5;

/// `name` as `(first, rest)`; `rest` is empty when the name fits on one line.
#[must_use]
pub fn name_lines(name: &str) -> (String, String) {
    let chars: Vec<char> = name.chars().collect();
    if chars.len() <= LINE_CHARS {
        return (name.to_owned(), String::new());
    }
    let at = (MIN_BREAK..=LINE_CHARS)
        .rev()
        .find(|&i| matches!(chars[i - 1], ' ' | '-' | '_'))
        .unwrap_or(LINE_CHARS);
    let first: String = chars[..at].iter().collect();
    let rest: String = chars[at..].iter().collect();
    (first.trim_end().to_owned(), rest.trim_start().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_short_name_stays_on_one_line() {
        assert_eq!(name_lines("final.png"), ("final.png".into(), String::new()));
        assert_eq!(
            name_lines("abcdefghijkl"),
            ("abcdefghijkl".into(), String::new())
        );
    }

    #[test]
    fn a_long_name_breaks_after_the_last_separator_that_fits() {
        assert_eq!(
            name_lines("Proje gorseli final.png"),
            ("Proje".into(), "gorseli final.png".into())
        );
        assert_eq!(
            name_lines("annual-report_2026.pdf"),
            ("annual-".into(), "report_2026.pdf".into())
        );
    }

    #[test]
    fn a_name_with_no_separator_breaks_at_the_limit() {
        assert_eq!(
            name_lines("ABCDEFGHIJKLMNOPQRST.zip"),
            ("ABCDEFGHIJKL".into(), "MNOPQRST.zip".into())
        );
    }

    #[test]
    fn the_split_counts_characters_not_bytes() {
        let (first, rest) = name_lines("ğüşiöçĞÜŞİÖÇ_abcdef.txt");
        assert_eq!(first.chars().count(), 12);
        assert!(rest.contains("abcdef"));
    }
}
