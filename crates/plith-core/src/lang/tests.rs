use super::*;
use crate::pages::audio::DeviceKind;
use crate::settings::LangChoice;

#[test]
fn every_text_has_every_language() {
    for lang in Lang::ALL {
        for t in Text::ALL {
            let own = match lang {
                Lang::En => en::text(t),
                Lang::Tr => tr::text(t),
            };
            let s = own.unwrap_or_else(|| panic!("{lang:?} lacks {t:?}"));
            assert!(!s.is_empty(), "{lang:?} {t:?} is empty");
            assert_eq!(upper(s, lang), s, "{lang:?} {t:?} is not upper-case");
            assert_eq!(text(t, lang), s);
        }
    }
}

#[test]
fn auto_follows_windows() {
    assert_eq!(resolve(&LangChoice::Auto, "tr-TR"), Lang::Tr);
    assert_eq!(resolve(&LangChoice::Auto, "de-DE"), Lang::En);
    assert_eq!(resolve(&LangChoice::Auto, "en_US"), Lang::En);
    assert_eq!(resolve(&LangChoice::Code("en".into()), "tr-TR"), Lang::En);
    assert_eq!(resolve(&LangChoice::Code("TR".into()), "en-US"), Lang::Tr);
    assert_eq!(resolve(&LangChoice::Code("xx".into()), "tr-TR"), Lang::En);
}

#[test]
fn lang_code_round_trips() {
    for lang in Lang::ALL {
        assert_eq!(resolve(&LangChoice::Code(lang.code().into()), ""), lang);
    }
}

#[test]
fn songs_plural_english() {
    assert_eq!(songs(1, Lang::En), "1 SONG");
    assert_eq!(songs(0, Lang::En), "0 SONGS");
    assert_eq!(songs(3, Lang::En), "3 SONGS");
    assert_eq!(songs(1, Lang::Tr), "1 ŞARKI");
    assert_eq!(songs(3, Lang::Tr), "3 ŞARKI");
}

#[test]
fn dates_both_languages() {
    assert_eq!(date_short(1, 17, 11, Lang::Tr), "SAL 17 KAS");
    assert_eq!(date_short(1, 17, 11, Lang::En), "TUE 17 NOV");
    assert_eq!(date_short(0, 5, 1, Lang::Tr), "PZT 5 OCA");
    assert_eq!(date_short(6, 31, 12, Lang::En), "SUN 31 DEC");
}

#[test]
fn upper_turkish_dotted_i() {
    assert_eq!(upper("çalan şarkı", Lang::Tr), "ÇALAN ŞARKI");
    assert_eq!(upper("izle", Lang::Tr), "İZLE");
    assert_eq!(upper("display", Lang::En), "DISPLAY");
}

#[test]
fn weekday_counts_from_monday() {
    assert_eq!(weekday((2026, 11, 17)), 1, "a Tuesday");
    assert_eq!(weekday((2026, 11, 16)), 0, "a Monday");
    assert_eq!(weekday((2024, 2, 29)), 3, "a Thursday, after a leap day");
    assert_eq!(weekday((2026, 1, 1)), 3);
    assert_eq!(weekday((2026, 11, 22)), 6, "a Sunday");
}

#[test]
fn sky_word_follows_the_language() {
    assert_eq!(sky_word(Sky::Rain, Lang::Tr), "YAĞMUR");
    assert_eq!(sky_word(Sky::Rain, Lang::En), "RAIN");
}

#[test]
fn device_kind_follows_the_language() {
    assert_eq!(device_kind(DeviceKind::Headphones, Lang::En), "HEADPHONES");
    assert_eq!(device_kind(DeviceKind::Headphones, Lang::Tr), "KULAKLIK");
    assert_eq!(device_kind(DeviceKind::Speaker, Lang::En), "SPEAKER");
    assert_eq!(device_kind(DeviceKind::Display, Lang::Tr), "EKRAN");
}
