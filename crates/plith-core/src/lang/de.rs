//! German strings. Upper-case, with `SS` instead of `ß`.

use super::Text;

/// Weekday abbreviations, Monday first.
pub const WEEKDAYS: [&str; 7] = ["MO", "DI", "MI", "DO", "FR", "SA", "SO"];

/// Month abbreviations, January first.
pub const MONTHS: [&str; 12] = [
    "JAN", "FEB", "MÄR", "APR", "MAI", "JUN", "JUL", "AUG", "SEP", "OKT", "NOV", "DEZ",
];

/// The German string for `t`.
pub fn text(t: Text) -> &'static str {
    match t {
        Text::LookKarot => "KERN",
        Text::LookClock => "UHR",
        Text::LookPlaying => "LÄUFT",
        Text::LookSeal => "TON",
        Text::LookPlain => "SCHLICHT",
        Text::LookHidden => "VERSTECKT",
        Text::NothingPlayed => "HEUTE NICHTS GESPIELT",
        Text::Read => "STAND",
        Text::Old => "ALT",
        Text::Forecast => "PROGNOSE",
        Text::Earlier => "FRÜHER",
        Text::Paused => "PAUSE",
        Text::SkyClear => "KLAR",
        Text::SkyCloud => "WOLKIG",
        Text::SkyRain => "REGEN",
        Text::SkySnow => "SCHNEE",
        Text::DefaultOutput => "STANDARDAUSGABE",
        Text::Off => "AUS",
        Text::KindHeadphones => "KOPFHÖRER",
        Text::KindSpeaker => "LAUTSPRECHER",
        Text::KindDisplay => "BILDSCHIRM",
        Text::TabAudio => "AUDIO",
        Text::TabMedia => "MEDIEN",
        Text::TabShelf => "ABLAGE",
        Text::TabSystem => "SYSTEM",
        Text::Muted => "STUMM",
        Text::DdcOff => "NICHT UNTERSTÜTZT · DDC/CI AM MONITOR AUS",
        Text::ShelfHint => "LIEGT IN DER ABLAGE, ZIEH ES ÜBERALLHIN ZURÜCK",
        Text::DefaultCaption => "STANDARD",
        Text::OtherOutputs => "WEITERE AUSGÄNGE",
        Text::NoOutput => "KEIN AUSGANG",
        Text::FooterNormal => "QUELLE, DANN AUSGANG KLICKEN",
        Text::FooterPicking => "AUSGANG KLICKEN: ROUTE · QUELLE ERNEUT KLICKEN: ABBRECHEN",
        Text::FooterOutputs => "AUSGANG KLICKEN: WIRD STANDARD",
        Text::Unused => "Unbenutzt",
        Text::ShelfDrop => "Dateien hierher ziehen",
        Text::Brightness => "Helligkeit",
        Text::Microphone => "Mikrofon",
        Text::NothingPlaying => "Es läuft nichts",
        Text::MediaHint => {
            "Erscheint hier, sobald Spotify, ein Browser oder ein Spiel Ton ausgibt."
        }
        Text::Source => "Quelle",
        Text::ShelfSlot => "ABLEGEN",
        Text::ShelfUndo => "RÜCKGÄNGIG",
        Text::ShelfClear => "LEEREN",
        Text::ShelfFooter => "ZIEHEN, ABLEGEN",
        Text::ShelfFolder => "ORDNER",
        Text::ShelfFile => "DATEI",
    }
}
