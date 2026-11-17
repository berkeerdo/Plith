//! Interface languages and the text table. Every UI string comes from here.

use crate::settings::LangChoice;
use crate::weather::Sky;

mod en;
mod tr;

#[cfg(test)]
mod tests;

/// A supported interface language. A new language adds a variant, a file and a `code` arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    /// English, the fallback.
    En,
    /// Turkish.
    Tr,
}

impl Lang {
    /// Every supported language.
    pub const ALL: [Lang; 2] = [Lang::En, Lang::Tr];

    /// The language code written to the settings file.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Tr => "tr",
        }
    }
}

/// A fixed interface string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Text {
    /// Name of the Karot look.
    LookKarot,
    /// Name of the Clock look.
    LookClock,
    /// Name of the Playing look.
    LookPlaying,
    /// Name of the Sound look.
    LookSeal,
    /// Name of the Plain look.
    LookPlain,
    /// Name of the Hidden look.
    LookHidden,
    /// Shown when nothing played today.
    NothingPlayed,
    /// Label before a reading time.
    Read,
    /// Marks a stale reading.
    Old,
    /// Label for a forecast.
    Forecast,
    /// Label for earlier plays.
    Earlier,
    /// Shown while playback is paused.
    Paused,
    /// Clear sky.
    SkyClear,
    /// Cloudy sky.
    SkyCloud,
    /// Rain.
    SkyRain,
    /// Snow.
    SkySnow,
    /// The default output device label.
    DefaultOutput,
    /// A muted output.
    Off,
}

impl Text {
    /// Every variant, for exhaustive checks.
    pub const ALL: [Text; 18] = [
        Text::LookKarot,
        Text::LookClock,
        Text::LookPlaying,
        Text::LookSeal,
        Text::LookPlain,
        Text::LookHidden,
        Text::NothingPlayed,
        Text::Read,
        Text::Old,
        Text::Forecast,
        Text::Earlier,
        Text::Paused,
        Text::SkyClear,
        Text::SkyCloud,
        Text::SkyRain,
        Text::SkySnow,
        Text::DefaultOutput,
        Text::Off,
    ];
}

/// Picks the language: an explicit code wins, `Auto` follows `windows_ui` (such as `tr-TR`).
/// Unknown languages resolve to English.
#[must_use]
pub fn resolve(choice: &LangChoice, windows_ui: &str) -> Lang {
    let tag = match choice {
        LangChoice::Auto => windows_ui,
        LangChoice::Code(code) => code,
    };
    let primary = tag.split(['-', '_']).next().unwrap_or("").to_lowercase();
    Lang::ALL
        .into_iter()
        .find(|lang| lang.code() == primary)
        .unwrap_or(Lang::En)
}

/// The string for `t` in `lang`, falling back to English when the language has none.
#[must_use]
pub fn text(t: Text, lang: Lang) -> &'static str {
    let own = match lang {
        Lang::En => en::text(t),
        Lang::Tr => tr::text(t),
    };
    own.or_else(|| en::text(t)).unwrap_or("")
}

/// A song count such as `3 ŞARKI`, `1 SONG` or `3 SONGS`.
#[must_use]
pub fn songs(n: usize, lang: Lang) -> String {
    match lang {
        Lang::Tr => format!("{n} ŞARKI"),
        Lang::En if n == 1 => format!("{n} SONG"),
        Lang::En => format!("{n} SONGS"),
    }
}

/// A short date such as `SAL 17 KAS` or `TUE 17 NOV`. `weekday` 0 is Monday, `month` 1 is January.
#[must_use]
pub fn date_short(weekday: u8, day: u16, month: u16, lang: Lang) -> String {
    let (weekdays, months) = match lang {
        Lang::En => (&en::WEEKDAYS, &en::MONTHS),
        Lang::Tr => (&tr::WEEKDAYS, &tr::MONTHS),
    };
    let wd = weekdays.get(usize::from(weekday)).copied().unwrap_or("");
    let mo = usize::from(month)
        .checked_sub(1)
        .and_then(|i| months.get(i))
        .copied()
        .unwrap_or("");
    format!("{wd} {day} {mo}")
}

/// The weekday of a calendar day, 0 for Monday.
#[must_use]
pub fn weekday((year, month, day): (u16, u16, u16)) -> u8 {
    const OFFSET: [u16; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let month = month.clamp(1, 12);
    let year = if month < 3 {
        year.saturating_sub(1)
    } else {
        year
    };
    let from_sunday =
        (year + year / 4 - year / 100 + year / 400 + OFFSET[usize::from(month - 1)] + day) % 7;
    // Allowed: the remainder is below 7.
    #[allow(clippy::cast_possible_truncation)]
    let from_sunday = from_sunday as u8;
    (from_sunday + 6) % 7
}

/// The word for a sky, for example `YAĞMUR`.
#[must_use]
pub fn sky_word(sky: Sky, lang: Lang) -> &'static str {
    text(
        match sky {
            Sky::Clear => Text::SkyClear,
            Sky::Cloud => Text::SkyCloud,
            Sky::Rain => Text::SkyRain,
            Sky::Snow => Text::SkySnow,
        },
        lang,
    )
}

/// Upper-cases `s` under the language's rules: Turkish maps `i` to `İ` and `ı` to `I`.
#[must_use]
pub fn upper(s: &str, lang: Lang) -> String {
    match lang {
        Lang::En => s.to_uppercase(),
        Lang::Tr => s
            .chars()
            .flat_map(|c| match c {
                'i' => 'İ'.to_uppercase(),
                'ı' => 'I'.to_uppercase(),
                other => other.to_uppercase(),
            })
            .collect(),
    }
}
