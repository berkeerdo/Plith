//! English strings. The fallback for every language: it covers every `Text`.

use super::Text;

/// Weekday abbreviations, Monday first.
pub const WEEKDAYS: [&str; 7] = ["MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN"];

/// Month abbreviations, January first.
pub const MONTHS: [&str; 12] = [
    "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
];

/// The English string for `t`.
#[allow(
    clippy::unnecessary_wraps,
    reason = "the shared signature lets a partial language return None"
)]
pub fn text(t: Text) -> Option<&'static str> {
    Some(match t {
        Text::LookKarot => "CORE",
        Text::LookClock => "CLOCK",
        Text::LookPlaying => "PLAYING",
        Text::LookSeal => "SOUND",
        Text::LookPlain => "PLAIN",
        Text::LookHidden => "HIDDEN",
        Text::NothingPlayed => "NOTHING PLAYED TODAY",
        Text::Read => "READ",
        Text::Old => "OLD",
        Text::Forecast => "FORECAST",
        Text::Earlier => "EARLIER",
        Text::Paused => "PAUSED",
        Text::SkyClear => "CLEAR",
        Text::SkyCloud => "CLOUDY",
        Text::SkyRain => "RAIN",
        Text::SkySnow => "SNOW",
        Text::DefaultOutput => "DEFAULT OUTPUT",
        Text::Off => "OFF",
        Text::KindHeadphones => "HEADPHONES",
        Text::KindSpeaker => "SPEAKER",
        Text::KindDisplay => "DISPLAY",
    })
}
