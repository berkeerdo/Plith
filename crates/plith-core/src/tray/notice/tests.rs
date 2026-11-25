use super::*;

#[test]
fn turkish_copy_is_the_spec() {
    assert_eq!(
        title(Version::new(0, 5, 0), Lang::Tr),
        "Plith 0.5.0 haz\u{131}r"
    );
    assert_eq!(body(Lang::Tr), "Kurmak i\u{e7}in t\u{131}kla");
}

#[test]
fn every_language_names_the_version_and_fits_a_balloon() {
    let version = Version::new(12, 345, 678);
    for lang in Lang::ALL {
        let title = title(version, lang);
        assert!(title.contains("12.345.678"), "{lang:?}: {title}");
        assert!(
            !title.contains('{'),
            "{lang:?}: placeholder left in {title}"
        );
        assert!(title.encode_utf16().count() <= 63, "{lang:?}");
        assert!(!body(lang).is_empty());
        assert!(body(lang).encode_utf16().count() <= 255, "{lang:?}");
    }
}

#[test]
fn the_copy_differs_between_languages() {
    let en = body(Lang::En);
    for lang in Lang::ALL.into_iter().filter(|l| *l != Lang::En) {
        assert_ne!(body(lang), en, "{lang:?}");
    }
}
