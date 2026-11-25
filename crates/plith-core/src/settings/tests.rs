use super::*;

#[test]
fn osd_and_flyout_round_trip() {
    let (s, problems) = parse("osd=off\nflyout=hide\n");
    assert!(problems.is_empty());
    assert!(!s.osd);
    assert_eq!(s.flyout, FlyoutPref::Hide);
    let text = serialize(&s);
    assert!(text.contains("osd=off\n"));
    assert!(text.contains("flyout=hide\n"));
    let defaults = serialize(&Settings::default());
    assert!(defaults.contains("osd=on\n"));
    assert!(defaults.contains("flyout=off\n"));
}

#[test]
fn defaults_are_karot_six_hours_weather_off() {
    let s = Settings::default();
    assert_eq!(s.look, IdleLook::Karot);
    assert_eq!(s.span, RunSpan::Six);
    assert!(s.plays);
    assert!(!s.weather);
    assert_eq!(s.location, LocationMode::Windows);
    assert!(s.clock24);
    assert_eq!(s.open_on, OpenOn::Welcome);
    assert_eq!(s.lang, LangChoice::Auto);
    assert_eq!(s.head, HeadSlot::Auto);
}

#[test]
fn round_trip() {
    let s = Settings {
        look: IdleLook::Seal,
        span: RunSpan::Day,
        plays: false,
        weather: true,
        location: LocationMode::City,
        city: "İstanbul".to_string(),
        unit: TempUnit::F,
        clock24: false,
        open_on: OpenOn::LastPage,
        lang: LangChoice::Code("tr".to_string()),
        head: HeadSlot::Playing,
        osd: false,
        flyout: FlyoutPref::Hide,
        theme: crate::theme::Theme::Sodyum,
        update_check: false,
        update_checked: 1_764_000_000,
        update_seen: "0.5.0".to_string(),
    };
    let (back, problems) = parse(&serialize(&s));
    assert_eq!(back, s);
    assert!(problems.is_empty());
}

#[test]
fn serialize_writes_every_key_in_order() {
    let text = serialize(&Settings::default());
    let keys: Vec<&str> = text.lines().map(|l| l.split('=').next().unwrap()).collect();
    assert_eq!(
        keys,
        [
            "look",
            "span",
            "plays",
            "weather",
            "location",
            "city",
            "unit",
            "clock24",
            "open_on",
            "lang",
            "head",
            "osd",
            "flyout",
            "theme",
            "update_check",
            "update_checked",
            "update_seen"
        ]
    );
    assert!(text.ends_with('\n'));
}

#[test]
fn unknown_key_and_bad_value_are_reported_and_ignored() {
    let (s, problems) = parse("look=eyes\nfoo=1\nweather=on\n");
    assert_eq!(s.look, IdleLook::Karot);
    assert!(s.weather);
    assert_eq!(problems.len(), 2);
}

#[test]
fn a_leading_byte_order_mark_does_not_hide_the_first_key() {
    let (s, problems) = parse("\u{feff}look=clock\nweather=on\n");
    assert_eq!(s.look, IdleLook::Clock);
    assert!(s.weather);
    assert!(problems.is_empty(), "{problems:?}");
}

#[test]
fn a_bad_value_is_reported_by_key_and_length_only() {
    let (_, problems) = parse("location=Ankara\n");
    assert_eq!(problems.len(), 1);
    assert!(problems[0].contains("`location`"));
    assert!(problems[0].contains("6 characters"));
    assert!(!problems[0].contains("Ankara"), "{problems:?}");
}

#[test]
fn blank_lines_and_hash_comments_are_skipped() {
    let (s, problems) = parse("\n  \n# a comment\nlook=clock\n   # indented\n");
    assert_eq!(s.look, IdleLook::Clock);
    assert!(problems.is_empty());
}

#[test]
fn keys_are_trimmed_and_case_sensitive() {
    let (s, problems) = parse("  look  =  plain  \nSpan=3h\n");
    assert_eq!(s.look, IdleLook::Plain);
    assert_eq!(s.span, RunSpan::Six);
    assert_eq!(problems.len(), 1);
}

#[test]
fn line_without_equals_is_a_problem() {
    let (_, problems) = parse("look\n");
    assert_eq!(problems.len(), 1);
}

#[test]
fn city_keeps_equals_and_non_ascii() {
    let (s, problems) = parse("city = İstanbul=Avrupa \n");
    assert_eq!(s.city, "İstanbul=Avrupa");
    assert!(problems.is_empty());
}

#[test]
fn unknown_lang_code_is_kept_without_a_problem() {
    let (s, problems) = parse("lang=de\n");
    assert_eq!(s.lang, LangChoice::Code("de".to_string()));
    assert!(problems.is_empty());
    assert_eq!(parse("lang=auto\n").0.lang, LangChoice::Auto);
}

#[test]
fn idle_look_keys_round_trip() {
    for look in [
        IdleLook::Karot,
        IdleLook::Clock,
        IdleLook::Playing,
        IdleLook::Seal,
        IdleLook::Plain,
        IdleLook::Hidden,
    ] {
        assert_eq!(IdleLook::from_key(look.key()), Some(look));
    }
    assert_eq!(IdleLook::from_key("eyes"), None);
}

#[test]
fn theme_defaults_to_gece_and_round_trips() {
    use crate::theme::Theme;
    assert_eq!(Settings::default().theme, Theme::Gece);
    assert!(serialize(&Settings::default()).contains("theme=gece\n"));
    for theme in Theme::ALL {
        let s = Settings {
            theme,
            ..Settings::default()
        };
        let (back, problems) = parse(&serialize(&s));
        assert_eq!(back.theme, theme);
        assert!(problems.is_empty());
    }
}

#[test]
fn unknown_theme_falls_back_to_gece_and_is_reported() {
    use crate::theme::Theme;
    let (s, problems) = parse("theme=neon\n");
    assert_eq!(s.theme, Theme::Gece);
    assert_eq!(problems.len(), 1);
    assert!(problems[0].contains("`theme`"));
    assert!(!problems[0].contains("neon"));
}

#[test]
fn settings_round_trip_update_keys() {
    let s = Settings {
        update_check: false,
        update_checked: 1_764_000_000,
        update_seen: "0.5.0".to_string(),
        ..Settings::default()
    };
    let text = serialize(&s);
    assert!(text.contains("update_checked=1764000000\n"));
    let (back, problems) = parse(&text);
    assert!(problems.is_empty());
    assert_eq!(back, s);
}

#[test]
fn old_settings_file_gets_update_defaults() {
    let (s, problems) = parse("look=clock\ntheme=gece\n");
    assert!(problems.is_empty());
    assert!(s.update_check);
    assert_eq!(s.update_checked, 0);
    assert_eq!(s.update_seen, "");
}

#[test]
fn bad_update_values_are_reported_without_the_value() {
    let (s, problems) = parse("update_seen=banana\nupdate_checked=-4\nupdate_check=maybe\n");
    assert_eq!(s.update_seen, "");
    assert_eq!(s.update_checked, 0);
    assert!(s.update_check);
    assert_eq!(problems.len(), 3);
    assert!(problems.iter().all(|p| !p.contains("banana")));
    assert!(problems[0].contains("`update_seen`"));
}
