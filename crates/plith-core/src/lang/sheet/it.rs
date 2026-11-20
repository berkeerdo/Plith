//! Italian settings window strings.

use super::SheetText;

/// The Italian string for `t`.
pub fn text(t: SheetText) -> &'static str {
    match t {
        SheetText::WindowTitle => "Plith · Impostazioni",
        SheetText::Caption => "IMPOSTAZIONI",
        SheetText::SectionGeneral => "Generale",
        SheetText::SectionIdle => "Riposo",
        SheetText::SectionOsd => "OSD",
        SheetText::SectionAbout => "Info",
        SheetText::LineGeneral => "Lingua e come si apre l'isola.",
        SheetText::LineIdle => "Cosa mostra l'isola chiusa, cosa leggi al clic.",
        SheetText::LineOsd => "Indicatore di volume e luminosità, sopra i giochi.",
        SheetText::LineAbout => "Versione e file.",
        SheetText::RowLanguage => "Lingua",
        SheetText::SubLanguage => "Automatica, segue la lingua di Windows",
        SheetText::RowOpens => "All'apertura dell'isola",
        SheetText::SubOpens => "Cosa compare per primo al clic",
        SheetText::CaptionClosed => "ISOLA CHIUSA",
        SheetText::PreviewNote => "Anteprima ridotta; sull'isola misura 200 x 32.",
        SheetText::RowStrip => "Fascia",
        SheetText::SubStrip => "Quante ore mostra il benvenuto",
        SheetText::RowTopRight => "In alto a destra",
        SheetText::SubTopRight => "Cosa sta in alto a destra nel benvenuto",
        SheetText::RowPlayed => "Ascoltati oggi",
        SheetText::SubPlayed => "Sopra la fascia, salvati solo su questo PC",
        SheetText::RowWeather => "Meteo",
        SheetText::SubWeather => "Se attivo, la posizione va a Open-Meteo",
        SheetText::RowLocation => "Posizione",
        SheetText::SubLocation => "Posizione di Windows, altrimenti IP, o manuale",
        SheetText::RowUnit => "Unità",
        SheetText::RowOsd => "Indicatore a schermo",
        SheetText::SubOsd => "Al cambio di volume o luminosità, sopra i giochi",
        SheetText::RowHideWindows => "Nascondi l'indicatore di Windows",
        SheetText::SubHideWindows => {
            "Nasconde l'indicatore di Windows mentre quello di Plith è attivo"
        }
        SheetText::RowVersion => "Versione",
        SheetText::RowFile => "File delle impostazioni",
        SheetText::RowData => "Dati",
        SheetText::SubData => "Meteo da Open-Meteo, posizione IP da ipapi.co (solo se scelta)",
        SheetText::WordAuto => "AUTOMATICO",
        SheetText::WordWelcome => "BENVENUTO",
        SheetText::WordLastPage => "ULTIMA",
        SheetText::WordHours3 => "±3 H",
        SheetText::WordHours6 => "±6 H",
        SheetText::WordAllDay => "GIORNATA",
        SheetText::WordWeather => "METEO",
        SheetText::WordSound => "SUONO",
        SheetText::WordPlaying => "IN ASCOLTO",
        SheetText::WordEmpty => "VUOTO",
        SheetText::WordWindows => "WINDOWS",
        SheetText::WordIp => "IP",
        SheetText::WordCity => "CITTÀ",
        SheetText::WordCelsius => "°C",
        SheetText::WordFahrenheit => "°F",
        SheetText::WordOn => "ATTIVO",
        SheetText::WordOff => "SPENTO",
        SheetText::WordOpen => "APRI",
        SheetText::WordAllSettings => "TUTTE LE OPZIONI",
        SheetText::SaveError => {
            "Impossibile salvare le impostazioni, valide solo per questa sessione."
        }
    }
}
