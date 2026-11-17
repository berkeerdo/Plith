//! Turkish strings. Upper-case under Turkish rules.

use super::Text;

/// Weekday abbreviations, Monday first.
pub const WEEKDAYS: [&str; 7] = ["PZT", "SAL", "ÇAR", "PER", "CUM", "CMT", "PAZ"];

/// Month abbreviations, January first.
pub const MONTHS: [&str; 12] = [
    "OCA", "ŞUB", "MAR", "NİS", "MAY", "HAZ", "TEM", "AĞU", "EYL", "EKİ", "KAS", "ARA",
];

/// The Turkish string for `t`, or `None` to fall back to English.
#[allow(
    clippy::unnecessary_wraps,
    reason = "the shared signature lets a partial language return None"
)]
pub fn text(t: Text) -> Option<&'static str> {
    Some(match t {
        Text::LookKarot => "KAROT",
        Text::LookClock => "SAAT",
        Text::LookPlaying => "ÇALAN",
        Text::LookSeal => "SES",
        Text::LookPlain => "SADE",
        Text::LookHidden => "GİZLİ",
        Text::NothingPlayed => "BUGÜN ÇALAN YOK",
        Text::Read => "OKUMA",
        Text::Old => "ESKİ",
        Text::Forecast => "TAHMİN",
        Text::Earlier => "GEÇMİŞ",
        Text::Paused => "DURDU",
        Text::SkyClear => "AÇIK",
        Text::SkyCloud => "BULUTLU",
        Text::SkyRain => "YAĞMUR",
        Text::SkySnow => "KAR",
    })
}
