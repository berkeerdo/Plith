//! The tray menu: what it holds, in which order, and the words in every language.

use crate::lang::Lang;

/// A command the tray can give.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayItem {
    /// Open the settings window (also what a left click on the icon does).
    Settings,
    /// Close the app, as `WM_CLOSE` does.
    Quit,
}

impl TrayItem {
    /// The menu command id: never 0, which a cancelled menu returns.
    #[must_use]
    pub fn id(self) -> u16 {
        match self {
            TrayItem::Settings => 1,
            TrayItem::Quit => 2,
        }
    }

    /// The item behind a menu command id.
    #[must_use]
    pub fn from_id(id: u16) -> Option<TrayItem> {
        match id {
            1 => Some(TrayItem::Settings),
            2 => Some(TrayItem::Quit),
            _ => None,
        }
    }
}

/// One line of the popup menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuLine {
    /// A command with its caption.
    Item(TrayItem, &'static str),
    /// A separator.
    Separator,
}

/// The menu in order: settings, a separator, quit.
#[must_use]
pub fn menu(lang: Lang) -> [MenuLine; 3] {
    [
        MenuLine::Item(TrayItem::Settings, label(TrayItem::Settings, lang)),
        MenuLine::Separator,
        MenuLine::Item(TrayItem::Quit, label(TrayItem::Quit, lang)),
    ]
}

/// The caption of `item` in `lang`. Sentence case: a native Windows menu is not upper case.
#[must_use]
pub fn label(item: TrayItem, lang: Lang) -> &'static str {
    match (item, lang) {
        (TrayItem::Settings, Lang::En) => "Settings",
        (TrayItem::Settings, Lang::Tr) => "Ayarlar",
        (TrayItem::Settings, Lang::De) => "Einstellungen",
        (TrayItem::Settings, Lang::Es) => "Ajustes",
        (TrayItem::Settings, Lang::Fr) => "Param\u{e8}tres",
        (TrayItem::Settings, Lang::Pt) => "Configura\u{e7}\u{f5}es",
        (TrayItem::Settings, Lang::It) => "Impostazioni",
        (TrayItem::Settings, Lang::Pl) => "Ustawienia",
        (TrayItem::Quit, Lang::En) => "Quit",
        (TrayItem::Quit, Lang::Tr) => "\u{c7}\u{131}k\u{131}\u{15f}",
        (TrayItem::Quit, Lang::De) => "Beenden",
        (TrayItem::Quit, Lang::Es) => "Salir",
        (TrayItem::Quit, Lang::Fr) => "Quitter",
        (TrayItem::Quit, Lang::Pt) => "Sair",
        (TrayItem::Quit, Lang::It) => "Esci",
        (TrayItem::Quit, Lang::Pl) => "Zako\u{144}cz",
    }
}

#[cfg(test)]
mod tests;
