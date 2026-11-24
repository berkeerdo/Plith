//! Portuguese settings window strings.

use super::SheetText;

/// The Portuguese string for `t`.
pub fn text(t: SheetText) -> &'static str {
    match t {
        SheetText::WindowTitle => "Plith · Configurações",
        SheetText::Caption => "CONFIGURAÇÕES",
        SheetText::SectionGeneral => "Geral",
        SheetText::SectionLook => "Aparência",
        SheetText::SectionIdle => "Repouso",
        SheetText::SectionOsd => "OSD",
        SheetText::SectionAbout => "Sobre",
        SheetText::LineGeneral => "Idioma e como a ilha abre.",
        SheetText::LineLook => "Cores da ilha e desta janela.",
        SheetText::LineIdle => "O que a ilha fechada mostra, o que você lê ao clicar.",
        SheetText::LineOsd => "Indicador de volume e brilho, sobre os jogos.",
        SheetText::LineAbout => "Versão e arquivos.",
        SheetText::RowLanguage => "Idioma",
        SheetText::SubLanguage => "Automático, segue o idioma do Windows",
        SheetText::RowOpens => "Ao abrir a ilha",
        SheetText::SubOpens => "O que aparece primeiro ao clicar",
        SheetText::RowTheme => "Tema",
        SheetText::SubTheme => "A ilha e esta janela mudam na hora",
        SheetText::CaptionClosed => "ILHA FECHADA",
        SheetText::PreviewNote => "Prévia reduzida; na ilha ela mede 200 x 32.",
        SheetText::RowStrip => "Faixa",
        SheetText::SubStrip => "Quantas horas as boas-vindas mostram",
        SheetText::RowTopRight => "No alto à direita",
        SheetText::SubTopRight => "O que fica no alto à direita das boas-vindas",
        SheetText::RowPlayed => "Tocadas hoje",
        SheetText::SubPlayed => "Acima da faixa, guardadas só neste PC",
        SheetText::RowWeather => "Clima",
        SheetText::SubWeather => "Ativado, a localização vai para o Open-Meteo",
        SheetText::RowLocation => "Localização",
        SheetText::SubLocation => "Localização do Windows, senão IP, ou manual",
        SheetText::RowUnit => "Unidade",
        SheetText::RowOsd => "Indicador na tela",
        SheetText::SubOsd => "Ao mudar volume ou brilho, sobre os jogos",
        SheetText::RowHideWindows => "Ocultar o indicador do Windows",
        SheetText::SubHideWindows => {
            "Oculta o indicador do próprio Windows enquanto o do Plith roda"
        }
        SheetText::RowAutostart => "Iniciar com o Windows",
        SheetText::SubAutostart => "O Plith abre sozinho quando você entra no Windows",
        SheetText::RowVersion => "Versão",
        SheetText::RowFile => "Arquivo de configurações",
        SheetText::RowData => "Dados",
        SheetText::SubData => {
            "Clima do Open-Meteo, localização por IP do ipapi.co (só se escolhida)"
        }
        SheetText::WordAuto => "AUTOMÁTICO",
        SheetText::WordWelcome => "INÍCIO",
        SheetText::WordLastPage => "ÚLTIMA",
        SheetText::WordHours3 => "±3 H",
        SheetText::WordHours6 => "±6 H",
        SheetText::WordAllDay => "O DIA TODO",
        SheetText::WordWeather => "CLIMA",
        SheetText::WordSound => "SOM",
        SheetText::WordPlaying => "TOCANDO",
        SheetText::WordEmpty => "VAZIO",
        SheetText::WordWindows => "WINDOWS",
        SheetText::WordIp => "IP",
        SheetText::WordCity => "CIDADE",
        SheetText::WordCelsius => "°C",
        SheetText::WordFahrenheit => "°F",
        SheetText::WordOn => "LIGADO",
        SheetText::WordOff => "DESLIGADO",
        SheetText::WordOpen => "ABRIR",
        SheetText::WordAllSettings => "TODAS AS OPÇÕES",
        SheetText::SaveError => "Não foi possível salvar as configurações, valem só nesta sessão.",
    }
}
