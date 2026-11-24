use super::*;

#[test]
fn the_menu_is_settings_separator_quit() {
    let lines = menu(Lang::Tr);
    assert_eq!(
        lines,
        [
            MenuLine::Item(TrayItem::Settings, "Ayarlar"),
            MenuLine::Separator,
            MenuLine::Item(TrayItem::Quit, "\u{c7}\u{131}k\u{131}\u{15f}"),
        ]
    );
}

#[test]
fn ids_round_trip_and_zero_is_nothing() {
    for item in [TrayItem::Settings, TrayItem::Quit] {
        assert_ne!(item.id(), 0);
        assert_eq!(TrayItem::from_id(item.id()), Some(item));
    }
    assert_eq!(TrayItem::from_id(0), None);
    assert_eq!(TrayItem::from_id(99), None);
}

#[test]
fn every_label_exists_in_every_language_and_differs_between_items() {
    for lang in Lang::ALL {
        let settings = label(TrayItem::Settings, lang);
        let quit = label(TrayItem::Quit, lang);
        assert!(!settings.trim().is_empty(), "{lang:?}");
        assert!(!quit.trim().is_empty(), "{lang:?}");
        assert_ne!(settings, quit, "{lang:?}");
    }
}

#[test]
fn every_language_but_english_has_its_own_word_for_settings() {
    for lang in Lang::ALL.into_iter().filter(|l| *l != Lang::En) {
        assert_ne!(
            label(TrayItem::Settings, lang),
            label(TrayItem::Settings, Lang::En),
            "{lang:?}"
        );
    }
}

#[test]
fn labels_are_sentence_case_not_upper_case() {
    // A native popup menu is not drawn in capitals (ADR 0012 covers drawn captions only).
    for lang in Lang::ALL {
        for item in [TrayItem::Settings, TrayItem::Quit] {
            let s = label(item, lang);
            assert_ne!(s, crate::lang::upper(s, lang), "{lang:?} {item:?}");
        }
    }
}

#[test]
fn the_tooltip_is_the_product_name() {
    assert_eq!(crate::tray::TOOLTIP, "Plith");
}
