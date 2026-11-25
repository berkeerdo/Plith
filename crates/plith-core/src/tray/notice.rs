//! The balloon that announces a new version: title and body in every language. Windows shows a
//! balloon title in 63 UTF-16 units and the body in 255; both stay far below.

use crate::lang::Lang;
use crate::lang::sheet::fill;
use crate::setup::Version;

/// The title, with `version` filled in.
#[must_use]
pub fn title(version: Version, lang: Lang) -> String {
    fill(title_copy(lang), "version", &version.to_string())
}

fn title_copy(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "Plith {version} is ready",
        Lang::Tr => "Plith {version} haz\u{131}r",
        Lang::De => "Plith {version} ist bereit",
        Lang::Es => "Plith {version} est\u{e1} listo",
        Lang::Fr => "Plith {version} est pr\u{ea}t",
        Lang::Pt => "Plith {version} est\u{e1} pronto",
        Lang::It => "Plith {version} \u{e8} pronto",
        Lang::Pl => "Plith {version} jest gotowy",
    }
}

/// The body: the click installs.
#[must_use]
pub fn body(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "Click to install",
        Lang::Tr => "Kurmak i\u{e7}in t\u{131}kla",
        Lang::De => "Zum Installieren klicken",
        Lang::Es => "Haz clic para instalar",
        Lang::Fr => "Cliquez pour installer",
        Lang::Pt => "Clique para instalar",
        Lang::It => "Fai clic per installare",
        Lang::Pl => "Kliknij, aby zainstalowa\u{107}",
    }
}

#[cfg(test)]
mod tests;
