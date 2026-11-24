//! Polish strings. Upper-case.

use super::Text;

/// Weekday abbreviations, Monday first.
pub const WEEKDAYS: [&str; 7] = ["PON", "WT", "ŚR", "CZW", "PT", "SOB", "ND"];

/// Month abbreviations, January first.
pub const MONTHS: [&str; 12] = [
    "STY", "LUT", "MAR", "KWI", "MAJ", "CZE", "LIP", "SIE", "WRZ", "PAŹ", "LIS", "GRU",
];

/// The Polish string for `t`.
pub fn text(t: Text) -> &'static str {
    match t {
        Text::LookKarot => "RDZEŃ",
        Text::LookClock => "ZEGAR",
        Text::LookPlaying => "ODTWARZA",
        Text::LookSeal => "DŹWIĘK",
        Text::LookPlain => "PROSTY",
        Text::LookHidden => "UKRYTY",
        Text::NothingPlayed => "DZIŚ NIC NIE GRANO",
        Text::Read => "ODCZYT",
        Text::Old => "STARY",
        Text::Forecast => "PROGNOZA",
        Text::Earlier => "WCZEŚNIEJ",
        Text::Paused => "PAUZA",
        Text::SkyClear => "JASNO",
        Text::SkyCloud => "POCHMURNO",
        Text::SkyRain => "DESZCZ",
        Text::SkySnow => "ŚNIEG",
        Text::DefaultOutput => "WYJŚCIE DOMYŚLNE",
        Text::Off => "WYŁ.",
        Text::KindHeadphones => "SŁUCHAWKI",
        Text::KindSpeaker => "GŁOŚNIK",
        Text::KindDisplay => "EKRAN",
        Text::TabAudio => "AUDIO",
        Text::TabMedia => "MEDIA",
        Text::TabShelf => "PÓŁKA",
        Text::TabSystem => "SYSTEM",
        Text::Muted => "CICHO",
        Text::DdcOff => "NIEOBSŁUGIWANE · DDC/CI WYŁĄCZONE W MONITORZE",
        Text::ShelfHint => "CZEKA NA PÓŁCE, PRZECIĄGNIJ Z POWROTEM",
        Text::DefaultCaption => "DOMYŚLNE",
        Text::OtherOutputs => "INNE WYJŚCIA",
        Text::NoOutput => "BRAK WYJŚCIA",
        Text::FooterNormal => "KLIKNIJ ŹRÓDŁO, POTEM WYJŚCIE",
        Text::FooterPicking => "KLIKNIJ WYJŚCIE: TRASA GOTOWA · KLIKNIJ ŹRÓDŁO: ANULUJ",
        Text::FooterOutputs => "KLIKNIJ WYJŚCIE: STAJE SIĘ DOMYŚLNE",
        Text::Unused => "Nieużywane",
        Text::ShelfDrop => "Przeciągnij pliki tutaj",
        Text::Brightness => "Jasność",
        Text::Microphone => "Mikrofon",
        Text::NothingPlaying => "Nic nie gra",
        Text::MediaHint => "Pojawi się tutaj, gdy Spotify, przeglądarka lub gra odtwarza dźwięk.",
        Text::Source => "Źródło",
        Text::ShelfSlot => "UPUŚĆ",
        Text::ShelfUndo => "COFNIJ",
        Text::ShelfClear => "WYCZYŚĆ",
        Text::ShelfFooter => "PRZECIĄGNIJ, UPUŚĆ",
        Text::ShelfFolder => "FOLDER",
        Text::ShelfFile => "PLIK",
    }
}
