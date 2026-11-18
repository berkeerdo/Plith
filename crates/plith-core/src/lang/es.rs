//! Spanish strings. Upper-case.

use super::Text;

/// Weekday abbreviations, Monday first.
pub const WEEKDAYS: [&str; 7] = ["LUN", "MAR", "MIÉ", "JUE", "VIE", "SÁB", "DOM"];

/// Month abbreviations, January first.
pub const MONTHS: [&str; 12] = [
    "ENE", "FEB", "MAR", "ABR", "MAY", "JUN", "JUL", "AGO", "SEP", "OCT", "NOV", "DIC",
];

/// The Spanish string for `t`.
pub fn text(t: Text) -> &'static str {
    match t {
        Text::LookKarot => "NÚCLEO",
        Text::LookClock => "RELOJ",
        Text::LookPlaying => "SONANDO",
        Text::LookSeal => "SONIDO",
        Text::LookPlain => "SIMPLE",
        Text::LookHidden => "OCULTO",
        Text::NothingPlayed => "NADA REPRODUCIDO HOY",
        Text::Read => "LECTURA",
        Text::Old => "VIEJO",
        Text::Forecast => "PRONÓSTICO",
        Text::Earlier => "ANTES",
        Text::Paused => "PAUSA",
        Text::SkyClear => "DESPEJADO",
        Text::SkyCloud => "NUBLADO",
        Text::SkyRain => "LLUVIA",
        Text::SkySnow => "NIEVE",
        Text::DefaultOutput => "SALIDA PRINCIPAL",
        Text::Off => "OFF",
        Text::KindHeadphones => "AURICULARES",
        Text::KindSpeaker => "ALTAVOZ",
        Text::KindDisplay => "PANTALLA",
        Text::TabAudio => "AUDIO",
        Text::TabMedia => "MEDIOS",
        Text::TabShelf => "ESTANTE",
        Text::TabSystem => "SISTEMA",
        Text::Muted => "MUDO",
        Text::DdcOff => "NO COMPATIBLE · DDC/CI DESACTIVADO EN EL MONITOR",
        Text::ShelfHint => "ESPERA EN EL ESTANTE, ARRÁSTRALO DE VUELTA A DONDE QUIERAS",
        Text::DefaultCaption => "PRINCIPAL",
        Text::OtherOutputs => "OTRAS SALIDAS",
        Text::NoOutput => "SIN SALIDA",
        Text::FooterNormal => "CLIC EN LA FUENTE, LUEGO EN LA SALIDA",
        Text::FooterPicking => "CLIC EN UNA SALIDA: SE ENRUTA · CLIC EN LA FUENTE: CANCELAR",
        Text::FooterOutputs => "CLIC EN UNA SALIDA: PASA A PRINCIPAL",
        Text::Unused => "Sin usar",
    }
}
