//! Portuguese strings (Brazilian). Upper-case.

use super::Text;

/// Weekday abbreviations, Monday first.
pub const WEEKDAYS: [&str; 7] = ["SEG", "TER", "QUA", "QUI", "SEX", "SÁB", "DOM"];

/// Month abbreviations, January first.
pub const MONTHS: [&str; 12] = [
    "JAN", "FEV", "MAR", "ABR", "MAI", "JUN", "JUL", "AGO", "SET", "OUT", "NOV", "DEZ",
];

/// The Portuguese string for `t`.
pub fn text(t: Text) -> &'static str {
    match t {
        Text::LookKarot => "NÚCLEO",
        Text::LookClock => "RELÓGIO",
        Text::LookPlaying => "TOCANDO",
        Text::LookSeal => "SOM",
        Text::LookPlain => "SIMPLES",
        Text::LookHidden => "OCULTO",
        Text::NothingPlayed => "NADA TOCOU HOJE",
        Text::Read => "LEITURA",
        Text::Old => "ANTIGO",
        Text::Forecast => "PREVISÃO",
        Text::Earlier => "ANTES",
        Text::Paused => "PAUSA",
        Text::SkyClear => "CLARO",
        Text::SkyCloud => "NUBLADO",
        Text::SkyRain => "CHUVA",
        Text::SkySnow => "NEVE",
        Text::DefaultOutput => "SAÍDA PADRÃO",
        Text::Off => "DESL.",
        Text::KindHeadphones => "FONES",
        Text::KindSpeaker => "ALTO-FALANTE",
        Text::KindDisplay => "TELA",
        Text::TabAudio => "ÁUDIO",
        Text::TabMedia => "MÍDIA",
        Text::TabShelf => "ESTANTE",
        Text::TabSystem => "SISTEMA",
        Text::Muted => "MUDO",
        Text::DdcOff => "SEM SUPORTE · DDC/CI DESLIGADO NO MONITOR",
        Text::ShelfHint => "FICA NA ESTANTE, ARRASTE DE VOLTA PARA ONDE QUISER",
        Text::DefaultCaption => "PADRÃO",
        Text::OtherOutputs => "OUTRAS SAÍDAS",
        Text::NoOutput => "SEM SAÍDA",
        Text::FooterNormal => "CLIQUE NA FONTE, DEPOIS NA SAÍDA",
        Text::FooterPicking => "CLIQUE NA SAÍDA: ROTA CRIADA · CLIQUE NA FONTE: CANCELAR",
        Text::FooterOutputs => "CLIQUE NA SAÍDA: VIRA PADRÃO",
        Text::Unused => "Sem uso",
        Text::ShelfDrop => "Arraste os arquivos para cá",
        Text::Brightness => "Brilho",
        Text::Microphone => "Microfone",
        Text::NothingPlaying => "Nada tocando",
        Text::MediaHint => "Aparece aqui quando o Spotify, um navegador ou um jogo emite som.",
        Text::Source => "Fonte",
        Text::ShelfSlot => "SOLTAR",
        Text::ShelfUndo => "DESFAZER",
        Text::ShelfClear => "LIMPAR",
        Text::ShelfFooter => "ARRASTE, SOLTE",
        Text::ShelfFolder => "PASTA",
        Text::ShelfFile => "ARQUIVO",
    }
}
