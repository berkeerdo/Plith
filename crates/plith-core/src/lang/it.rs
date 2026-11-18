//! Italian strings. Upper-case.

use super::Text;

/// Weekday abbreviations, Monday first.
pub const WEEKDAYS: [&str; 7] = ["LUN", "MAR", "MER", "GIO", "VEN", "SAB", "DOM"];

/// Month abbreviations, January first.
pub const MONTHS: [&str; 12] = [
    "GEN", "FEB", "MAR", "APR", "MAG", "GIU", "LUG", "AGO", "SET", "OTT", "NOV", "DIC",
];

/// The Italian string for `t`.
pub fn text(t: Text) -> &'static str {
    match t {
        Text::LookKarot => "NUCLEO",
        Text::LookClock => "OROLOGIO",
        Text::LookPlaying => "IN CORSO",
        Text::LookSeal => "SUONO",
        Text::LookPlain => "SEMPLICE",
        Text::LookHidden => "NASCOSTO",
        Text::NothingPlayed => "NIENTE ASCOLTATO OGGI",
        Text::Read => "LETTURA",
        Text::Old => "VECCHIO",
        Text::Forecast => "PREVISIONI",
        Text::Earlier => "PRIMA",
        Text::Paused => "PAUSA",
        Text::SkyClear => "SERENO",
        Text::SkyCloud => "NUVOLOSO",
        Text::SkyRain => "PIOGGIA",
        Text::SkySnow => "NEVE",
        Text::DefaultOutput => "USCITA PREDEFINITA",
        Text::Off => "OFF",
        Text::KindHeadphones => "CUFFIE",
        Text::KindSpeaker => "ALTOPARLANTE",
        Text::KindDisplay => "SCHERMO",
        Text::TabAudio => "AUDIO",
        Text::TabMedia => "MEDIA",
        Text::TabShelf => "MENSOLA",
        Text::TabSystem => "SISTEMA",
        Text::Muted => "MUTO",
        Text::DdcOff => "NON SUPPORTATO · DDC/CI SPENTO SUL MONITOR",
        Text::ShelfHint => "RESTA SULLA MENSOLA, TRASCINALO DOVE VUOI",
        Text::DefaultCaption => "PREDEFINITA",
        Text::OtherOutputs => "ALTRE USCITE",
        Text::NoOutput => "NESSUNA USCITA",
        Text::FooterNormal => "CLICCA LA SORGENTE, POI L'USCITA",
        Text::FooterPicking => "CLICCA UN'USCITA: SI COLLEGA · CLICCA ANCORA LA SORGENTE: ANNULLA",
        Text::FooterOutputs => "CLICCA UN'USCITA: DIVENTA PREDEFINITA",
        Text::Unused => "Inutilizzato",
    }
}
