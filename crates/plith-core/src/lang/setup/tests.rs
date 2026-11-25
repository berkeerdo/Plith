use super::*;
use crate::lang::upper;

#[test]
fn every_setup_string_in_eight_languages() {
    for lang in Lang::ALL {
        for t in SetupText::ALL {
            let s = setup_text(t, lang);
            assert!(!s.trim().is_empty(), "{lang:?} {t:?} is empty");
            if t.is_caption() {
                assert_eq!(s, upper(s, lang), "{lang:?} {t:?} is not upper case");
            }
        }
    }
}

#[test]
fn turkish_copy_exact() {
    let tr = |t| setup_text(t, Lang::Tr);
    assert_eq!(tr(SetupText::Step1), "DOSYALAR");
    assert_eq!(tr(SetupText::Step2), "YARDIMCI");
    assert_eq!(tr(SetupText::Step3), "BA\u{15e}LAT");
    assert_eq!(tr(SetupText::Install), "KUR");
    assert_eq!(tr(SetupText::Open), "PLITH\u{2019}\u{130} A\u{c7}");
    assert_eq!(tr(SetupText::Close), "KAPAT");
    assert_eq!(tr(SetupText::Autostart), "WINDOWS \u{130}LE BA\u{15e}LAT");
    assert_eq!(tr(SetupText::Update), "G\u{dc}NCELLE");
    assert_eq!(tr(SetupText::Retry), "TEKRAR DENE");
    assert_eq!(tr(SetupText::Uninstall), "KALDIR");
    assert_eq!(
        tr(SetupText::DeleteData),
        "KULLANICI VER\u{130}LER\u{130}N\u{130} DE S\u{130}L"
    );
}

#[test]
fn prose_is_plain() {
    for lang in Lang::ALL {
        for t in SetupText::ALL {
            let s = setup_text(t, lang);
            assert!(!s.contains('\u{2014}'), "{lang:?} {t:?} has an em dash");
            assert!(!s.contains('!'), "{lang:?} {t:?} has an exclamation mark");
        }
    }
}

#[test]
fn every_language_is_translated() {
    let en = setup_text(SetupText::LeadStart, Lang::En);
    for lang in Lang::ALL.into_iter().filter(|l| *l != Lang::En) {
        assert_ne!(setup_text(SetupText::LeadStart, lang), en, "{lang:?}");
    }
}
