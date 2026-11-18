//! French strings. Upper-case.

use super::Text;

/// Weekday abbreviations, Monday first.
pub const WEEKDAYS: [&str; 7] = ["LUN", "MAR", "MER", "JEU", "VEN", "SAM", "DIM"];

/// Month abbreviations, January first.
pub const MONTHS: [&str; 12] = [
    "JAN", "FÉV", "MAR", "AVR", "MAI", "JUIN", "JUIL", "AOÛ", "SEP", "OCT", "NOV", "DÉC",
];

/// The French string for `t`.
pub fn text(t: Text) -> &'static str {
    match t {
        Text::LookKarot => "NOYAU",
        Text::LookClock => "HORLOGE",
        Text::LookPlaying => "EN COURS",
        Text::LookSeal => "SON",
        Text::LookPlain => "SIMPLE",
        Text::LookHidden => "CACHÉ",
        Text::NothingPlayed => "RIEN ÉCOUTÉ AUJOURD'HUI",
        Text::Read => "RELEVÉ",
        Text::Old => "ANCIEN",
        Text::Forecast => "PRÉVISION",
        Text::Earlier => "AVANT",
        Text::Paused => "PAUSE",
        Text::SkyClear => "DÉGAGÉ",
        Text::SkyCloud => "NUAGEUX",
        Text::SkyRain => "PLUIE",
        Text::SkySnow => "NEIGE",
        Text::DefaultOutput => "SORTIE PAR DÉFAUT",
        Text::Off => "COUPÉ",
        Text::KindHeadphones => "CASQUE",
        Text::KindSpeaker => "ENCEINTE",
        Text::KindDisplay => "ÉCRAN",
        Text::TabAudio => "AUDIO",
        Text::TabMedia => "MÉDIAS",
        Text::TabShelf => "ÉTAGÈRE",
        Text::TabSystem => "SYSTÈME",
        Text::Muted => "MUET",
        Text::DdcOff => "NON PRIS EN CHARGE · DDC/CI DÉSACTIVÉ SUR L'ÉCRAN",
        Text::ShelfHint => "ATTEND SUR L'ÉTAGÈRE, REMETS-LE OÙ TU VEUX",
        Text::DefaultCaption => "PAR DÉFAUT",
        Text::OtherOutputs => "AUTRES SORTIES",
        Text::NoOutput => "AUCUNE SORTIE",
        Text::FooterNormal => "CLIQUE SUR LA SOURCE, PUIS LA SORTIE",
        Text::FooterPicking => "CLIQUE SUR UNE SORTIE : CRÉE LA ROUTE · SUR LA SOURCE : ANNULER",
        Text::FooterOutputs => "CLIQUE SUR UNE SORTIE : PAR DÉFAUT",
        Text::Unused => "Inutilisé",
        Text::ShelfDrop => "Glisse les fichiers ici",
        Text::Brightness => "Luminosité",
        Text::Microphone => "Microphone",
        Text::NothingPlaying => "Rien en lecture",
        Text::MediaHint => "Apparaît ici quand Spotify, un navigateur ou un jeu émet du son.",
        Text::Source => "Source",
    }
}
