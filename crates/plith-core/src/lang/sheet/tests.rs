use super::*;
use crate::lang::upper;

#[test]
fn every_sheet_text_in_every_language() {
    for lang in Lang::ALL {
        for t in SheetText::ALL {
            assert!(
                !sheet_text(t, lang).trim().is_empty(),
                "{lang:?} {t:?} is empty"
            );
        }
    }
}

#[test]
fn turkish_copy_exact() {
    let tr = |t| sheet_text(t, Lang::Tr);
    assert_eq!(tr(SheetText::WindowTitle), "Plith \u{b7} Ayarlar");
    assert_eq!(tr(SheetText::SectionAbout), "Hakk\u{131}nda");
    assert_eq!(
        tr(SheetText::SubWeather),
        "A\u{e7}\u{131}kken konum Open-Meteo'ya gider"
    );
    assert_eq!(tr(SheetText::WordHours3), "\u{b1}3 SA");
    assert_eq!(
        tr(SheetText::SaveError),
        "Ayarlar kaydedilemedi, bu oturumda ge\u{e7}erli."
    );
    assert_eq!(tr(SheetText::WordAllSettings), "T\u{dc}M AYARLAR");
}

#[test]
fn captions_are_upper() {
    for lang in Lang::ALL {
        for t in SheetText::ALL {
            if t.is_caption() {
                // The copy holds placeholders the code fills with a number.
                let s = fill(
                    &fill(sheet_text(t, lang), "version", "1.0.0"),
                    "percent",
                    "1",
                );
                assert_eq!(s, upper(&s, lang), "{lang:?} {t:?} is not upper case");
            }
        }
    }
}

#[test]
fn the_five_planned_theme_names_have_a_place_in_every_language() {
    use super::fit::THEME_NAMES;
    use crate::lang::fit::places;
    let lower = ["gece", "halka", "firn", "kablo", "\u{15f}erit"];
    for (name, low) in THEME_NAMES.into_iter().zip(lower) {
        assert_eq!(name, upper(low, Lang::Tr), "{name}");
    }
    for lang in Lang::ALL {
        let all = places(lang);
        for name in THEME_NAMES {
            assert!(
                all.iter().any(|p| p.text == name),
                "{lang:?} {name} has no place in the fit list"
            );
        }
    }
}

#[test]
fn the_look_section_copy_is_per_language() {
    assert_eq!(
        sheet_text(SheetText::SectionLook, Lang::Tr),
        "G\u{f6}r\u{fc}n\u{fc}m"
    );
    assert_eq!(sheet_text(SheetText::RowTheme, Lang::Tr), "Tema");
    let en = sheet_text(SheetText::SectionLook, Lang::En);
    for lang in Lang::ALL.into_iter().filter(|l| *l != Lang::En) {
        assert_ne!(sheet_text(SheetText::SectionLook, lang), en, "{lang:?}");
    }
}

#[test]
fn language_names_are_in_themselves() {
    assert_eq!(lang_name(Lang::Tr), "T\u{fc}rk\u{e7}e");
    assert_eq!(lang_name(Lang::De), "Deutsch");
    for lang in Lang::ALL {
        assert!(!lang_name(lang).is_empty());
    }
}

#[test]
fn no_em_dash_anywhere() {
    for lang in Lang::ALL {
        for t in SheetText::ALL {
            assert!(!sheet_text(t, lang).contains('\u{2014}'), "{lang:?} {t:?}");
        }
    }
}
