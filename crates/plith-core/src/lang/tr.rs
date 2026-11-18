//! Turkish strings. Upper-case under Turkish rules.

use super::Text;

/// Weekday abbreviations, Monday first.
pub const WEEKDAYS: [&str; 7] = ["PZT", "SAL", "ÇAR", "PER", "CUM", "CMT", "PAZ"];

/// Month abbreviations, January first.
pub const MONTHS: [&str; 12] = [
    "OCA", "ŞUB", "MAR", "NİS", "MAY", "HAZ", "TEM", "AĞU", "EYL", "EKİ", "KAS", "ARA",
];

/// The Turkish string for `t`.
pub fn text(t: Text) -> &'static str {
    match t {
        Text::LookKarot => "KAROT",
        Text::LookClock => "SAAT",
        Text::LookPlaying => "ÇALAN",
        Text::LookSeal | Text::TabAudio => "SES",
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
        Text::DefaultOutput => "VARSAYILAN ÇIKIŞ",
        Text::Off => "KAPALI",
        Text::KindHeadphones => "KULAKLIK",
        Text::KindSpeaker => "HOPARLÖR",
        Text::KindDisplay => "EKRAN",
        Text::TabMedia => "MEDYA",
        Text::TabShelf => "RAF",
        Text::TabSystem => "SİSTEM",
        Text::Muted => "SESSİZ",
        Text::DdcOff => "DESTEKLENMİYOR · MONİTÖRDE DDC/CI KAPALI",
        Text::ShelfHint => "RAFTA BEKLER, İSTEDİĞİN YERE GERİ SÜRÜKLERSİN",
        Text::DefaultCaption => "VARSAYILAN",
        Text::OtherOutputs => "DİĞER ÇIKIŞLAR",
        Text::NoOutput => "ÇIKIŞ YOK",
        Text::FooterNormal => "KAYNAĞA, SONRA ÇIKIŞA TIKLA",
        Text::FooterPicking => "ÇIKIŞA TIKLA: YOL KURULUR · KAYNAĞA TEKRAR TIKLA: VAZGEÇ",
        Text::FooterOutputs => "ÇIKIŞA TIKLA: VARSAYILAN OLUR",
        Text::Unused => "Kullanılmıyor",
    }
}
