use super::*;
use crate::pages::audio::DeviceKind;
use crate::settings::LangChoice;

#[test]
fn auto_follows_windows() {
    assert_eq!(resolve(&LangChoice::Auto, "tr-TR"), Lang::Tr);
    assert_eq!(resolve(&LangChoice::Auto, "ja-JP"), Lang::En);
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
fn french_zero_and_one_are_singular() {
    assert_eq!(songs(0, Lang::Fr), "0 TITRE");
    assert_eq!(songs(1, Lang::Fr), "1 TITRE");
    assert_eq!(songs(2, Lang::Fr), "2 TITRES");
}

#[test]
fn file_counts_follow_each_language() {
    assert_eq!(files(4, Lang::Tr), "4 DOSYA");
    assert_eq!(files(1, Lang::En), "1 FILE");
    assert_eq!(files(8, Lang::En), "8 FILES");
    assert_eq!(files(1, Lang::De), "1 DATEI");
    assert_eq!(files(2, Lang::De), "2 DATEIEN");
    assert_eq!(files(3, Lang::It), "3 FILE");
    assert_eq!(files(1, Lang::Pl), "1 PLIK");
    assert_eq!(files(3, Lang::Pl), "3 PLIKI");
    assert_eq!(files(5, Lang::Pl), "5 PLIKÓW");
    assert_eq!(files(8, Lang::Pl), "8 PLIKÓW");
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
#[test]
fn every_language_has_every_string() {
    for lang in Lang::ALL {
        for t in Text::ALL {
            assert!(!text(t, lang).is_empty(), "{lang:?} {t:?} is empty");
        }
    }
}

/// The strings written in sentence case: the unused bus stands in for a bus name, the rest are
/// page sentences and row names, not labels.
const SENTENCE_CASE: [Text; 7] = [
    Text::Unused,
    Text::ShelfDrop,
    Text::Brightness,
    Text::Microphone,
    Text::NothingPlaying,
    Text::MediaHint,
    Text::Source,
];

#[test]
fn own_copy_is_upper_case() {
    for lang in Lang::ALL {
        for t in Text::ALL {
            if SENTENCE_CASE.contains(&t) {
                continue;
            }
            let s = text(t, lang);
            assert_eq!(upper(s, lang), s, "{lang:?} {t:?} is not upper-case");
            if lang == Lang::De {
                assert!(!s.contains('ß'), "{t:?} keeps a sharp s");
            }
        }
    }
}

#[test]
fn muted_and_off_words_fit_the_value_slot() {
    for lang in Lang::ALL {
        for t in [Text::Muted, Text::Off] {
            assert!(text(t, lang).chars().count() <= 6, "{lang:?} {t:?} is long");
        }
    }
}

#[test]
fn turkish_copy_is_unchanged() {
    let expected = [
        (Text::TabAudio, "SES"),
        (Text::TabMedia, "MEDYA"),
        (Text::TabShelf, "RAF"),
        (Text::TabSystem, "SİSTEM"),
        (Text::Muted, "SESSİZ"),
        (Text::Off, "KAPALI"),
        (Text::DdcOff, "DESTEKLENMİYOR · MONİTÖRDE DDC/CI KAPALI"),
        (
            Text::ShelfHint,
            "RAFTA BEKLER, İSTEDİĞİN YERE GERİ SÜRÜKLERSİN",
        ),
        (Text::DefaultCaption, "VARSAYILAN"),
        (Text::OtherOutputs, "DİĞER ÇIKIŞLAR"),
        (Text::NoOutput, "ÇIKIŞ YOK"),
        (Text::FooterNormal, "KAYNAĞA, SONRA ÇIKIŞA TIKLA"),
        (
            Text::FooterPicking,
            "ÇIKIŞA TIKLA: YOL KURULUR · KAYNAĞA TEKRAR TIKLA: VAZGEÇ",
        ),
        (Text::FooterOutputs, "ÇIKIŞA TIKLA: VARSAYILAN OLUR"),
        (Text::Unused, "Kullanılmıyor"),
        (Text::ShelfDrop, "Dosyaları buraya sürükle"),
        (Text::Brightness, "Parlaklık"),
        (Text::Microphone, "Mikrofon"),
        (Text::NothingPlaying, "Çalan bir şey yok"),
        (
            Text::MediaHint,
            "Spotify, tarayıcı ya da bir oyun ses verdiğinde burada görünür.",
        ),
        (Text::Source, "Kaynak"),
        (Text::ShelfFolder, "KLASÖR"),
        (Text::ShelfFile, "DOSYA"),
        (Text::ShelfSlot, "BIRAK"),
        (Text::ShelfUndo, "GERİ AL"),
        (Text::ShelfFooter, "SÜRÜKLE, BIRAK"),
    ];
    for (t, s) in expected {
        assert_eq!(text(t, Lang::Tr), s, "{t:?}");
    }
}

#[test]
fn percent_follows_the_language() {
    let expected = [
        (Lang::Tr, "%62"),
        (Lang::En, "62%"),
        (Lang::De, "62 %"),
        (Lang::Fr, "62 %"),
        (Lang::Es, "62 %"),
        (Lang::Pt, "62%"),
        (Lang::It, "62%"),
        (Lang::Pl, "62%"),
    ];
    for (lang, s) in expected {
        assert_eq!(percent(62, lang), s, "{lang:?}");
    }
}

#[test]
fn regions_resolve_by_primary_code() {
    let auto = |tag| resolve(&LangChoice::Auto, tag);
    assert_eq!(auto("pt-PT"), Lang::Pt);
    assert_eq!(auto("de-CH"), Lang::De);
    assert_eq!(auto("fr-CA"), Lang::Fr);
    assert_eq!(auto("xx-YY"), Lang::En);
    assert_eq!(resolve(&LangChoice::Code("pl".into()), "en-US"), Lang::Pl);
}

#[test]
fn polish_counts_follow_the_teen_rule() {
    let expected = [
        (1, "1 UTWÓR"),
        (2, "2 UTWORY"),
        (4, "4 UTWORY"),
        (5, "5 UTWORÓW"),
        (12, "12 UTWORÓW"),
        (14, "14 UTWORÓW"),
        (22, "22 UTWORY"),
        (25, "25 UTWORÓW"),
    ];
    for (n, s) in expected {
        assert_eq!(songs(n, Lang::Pl), s);
    }
}

#[test]
fn dates_exist_in_every_language() {
    for lang in Lang::ALL {
        let date = date_short(1, 17, 11, lang);
        let parts: Vec<&str> = date.split(' ').collect();
        assert_eq!(parts.len(), 3, "{lang:?}: {date}");
        assert!(parts.iter().all(|p| !p.is_empty()), "{lang:?}: {date}");
        for weekday in 0..7 {
            let word = date_short(weekday, 1, 1, lang);
            let word = word.split(' ').next().unwrap_or("");
            assert_eq!(upper(word, lang), word, "{lang:?} weekday {weekday}");
        }
        for month in 1..=12 {
            let date = date_short(0, 1, month, lang);
            let word = date.rsplit(' ').next().unwrap_or("");
            assert_eq!(upper(word, lang), word, "{lang:?} month {month}");
        }
    }
}
