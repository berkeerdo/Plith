//! Interface languages and the text table. Every UI string comes from here.

use crate::settings::LangChoice;

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
}

impl Text {
    /// Every variant, for exhaustive checks.
    pub const ALL: [Text; 16] = [
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
