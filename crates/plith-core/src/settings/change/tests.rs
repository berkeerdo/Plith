use super::*;

fn only(f: impl FnOnce(&mut Effects)) -> Effects {
    let mut e = Effects::default();
    f(&mut e);
    e
}

#[test]
fn apply_change_effects() {
    let cases = [
        (
            SettingChange::Look(IdleLook::Clock),
            only(|e| e.pill = true),
        ),
        (
            SettingChange::Span(RunSpan::Day),
            only(|e| e.welcome = true),
        ),
        (
            SettingChange::Head(HeadSlot::Empty),
            only(|e| e.welcome = true),
        ),
        (SettingChange::Plays(false), only(|e| e.welcome = true)),
        (
            SettingChange::Unit(TempUnit::F),
            only(|e| {
                e.welcome = true;
                e.pill = true;
            }),
        ),
        (
            SettingChange::OpenOn(OpenOn::LastPage),
            only(|e| e.welcome = true),
        ),
        (SettingChange::Weather(true), only(|e| e.weather = true)),
        (
            SettingChange::Location(LocationMode::Ip),
            only(|e| e.weather = true),
        ),
        (
            SettingChange::City("Ankara".to_string()),
            only(|e| e.weather = true),
        ),
        (
            SettingChange::Lang(LangChoice::Code("tr".to_string())),
            only(|e| e.lang = true),
        ),
        (SettingChange::Osd(false), only(|e| e.osd = true)),
        (SettingChange::Theme(Theme::York), only(|e| e.theme = true)),
        (
            SettingChange::Flyout(FlyoutPref::Hide),
            only(|e| e.osd = true),
        ),
    ];
    for (change, expected) in cases {
        let mut s = Settings::default();
        let label = format!("{change:?}");
        assert_eq!(apply_change(&mut s, change), expected, "{label}");
    }
}

#[test]
fn applying_the_current_value_changes_nothing() {
    let current = Settings::default();
    let same = [
        SettingChange::Look(current.look),
        SettingChange::Span(current.span),
        SettingChange::Head(current.head),
        SettingChange::Plays(current.plays),
        SettingChange::Weather(current.weather),
        SettingChange::Location(current.location),
        SettingChange::City(current.city.clone()),
        SettingChange::Unit(current.unit),
        SettingChange::OpenOn(current.open_on),
        SettingChange::Lang(current.lang.clone()),
        SettingChange::Osd(current.osd),
        SettingChange::Flyout(current.flyout),
        SettingChange::Theme(current.theme),
        SettingChange::UpdateCheck(current.update_check),
    ];
    for change in same {
        let mut s = Settings::default();
        assert_eq!(apply_change(&mut s, change), Effects::default());
        assert_eq!(s, current);
    }
}

#[test]
fn a_change_is_stored() {
    let mut s = Settings::default();
    apply_change(&mut s, SettingChange::Osd(false));
    apply_change(&mut s, SettingChange::City("Izmir".to_string()));
    assert!(!s.osd);
    assert_eq!(s.city, "Izmir");
}

#[test]
fn update_check_change_flips_the_service() {
    let mut s = Settings::default();
    let effects = apply_change(&mut s, SettingChange::UpdateCheck(false));
    assert_eq!(effects, only(|e| e.update = true));
    assert!(!s.update_check);
}
