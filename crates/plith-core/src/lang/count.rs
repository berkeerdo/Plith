//! Counts and numbers: song counts per language and the percent format.

use super::Lang;

/// A song count such as `3 ŞARKI`, `1 SONG` or `3 SONGS`.
#[must_use]
pub fn songs(n: usize, lang: Lang) -> String {
    let (one, many) = match lang {
        Lang::En => ("SONG", "SONGS"),
        Lang::Tr => return format!("{n} ŞARKI"),
        Lang::De => ("LIED", "LIEDER"),
        Lang::Es => ("CANCIÓN", "CANCIONES"),
        Lang::Fr => ("TITRE", "TITRES"),
        Lang::Pt => ("MÚSICA", "MÚSICAS"),
        Lang::It => ("BRANO", "BRANI"),
        Lang::Pl => return format!("{n} {}", polish_songs(n)),
    };
    // French treats zero as singular too.
    let singular = n == 1 || (lang == Lang::Fr && n == 0);
    format!("{n} {}", if singular { one } else { many })
}

/// The Polish noun form for `n`: 1 singular, 2 to 4 few (not 12 to 14), the rest many.
fn polish_songs(n: usize) -> &'static str {
    let last = n % 10;
    let teen = (n % 100) / 10 == 1;
    if n == 1 {
        "UTWÓR"
    } else if (2..=4).contains(&last) && !teen {
        "UTWORY"
    } else {
        "UTWORÓW"
    }
}

/// A percentage as the language writes it: `%62` Turkish, `62 %` German, Spanish and French,
/// `62%` for the rest.
#[must_use]
pub fn percent(v: u8, lang: Lang) -> String {
    match lang {
        Lang::Tr => format!("%{v}"),
        Lang::De | Lang::Es | Lang::Fr => format!("{v} %"),
        Lang::En | Lang::Pt | Lang::It | Lang::Pl => format!("{v}%"),
    }
}
