//! English strings. The fallback for an unknown language code.

use super::Text;

/// Weekday abbreviations, Monday first.
pub const WEEKDAYS: [&str; 7] = ["MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN"];

/// Month abbreviations, January first.
pub const MONTHS: [&str; 12] = [
    "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
];

/// The English string for `t`.
pub fn text(t: Text) -> &'static str {
    match t {
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
        Text::TabAudio => "AUDIO",
        Text::TabMedia => "MEDIA",
        Text::TabShelf => "SHELF",
        Text::TabSystem => "SYSTEM",
        Text::Muted => "MUTED",
        Text::DdcOff => "UNSUPPORTED · DDC/CI IS OFF ON THE MONITOR",
        Text::ShelfHint => "IT WAITS ON THE SHELF, DRAG IT BACK ANYWHERE",
        Text::DefaultCaption => "DEFAULT",
        Text::OtherOutputs => "OTHER OUTPUTS",
        Text::NoOutput => "NO OUTPUT",
        Text::FooterNormal => "CLICK A SOURCE, THEN AN OUTPUT",
        Text::FooterPicking => "CLICK AN OUTPUT: ROUTES IT · CLICK THE SOURCE AGAIN: CANCEL",
        Text::FooterOutputs => "CLICK AN OUTPUT: MAKES IT DEFAULT",
        Text::Unused => "Unused",
        Text::ShelfDrop => "Drop files here",
        Text::Brightness => "Brightness",
        Text::Microphone => "Microphone",
        Text::NothingPlaying => "Nothing playing",
        Text::MediaHint => "Appears here when Spotify, a browser or a game makes sound.",
        Text::Source => "Source",
        Text::ShelfSlot => "DROP",
        Text::ShelfUndo => "UNDO",
        Text::ShelfClear => "EMPTY",
        Text::ShelfFooter => "DRAG, DROP",
        Text::ShelfFolder => "FOLDER",
        Text::ShelfFile => "FILE",
    }
}
