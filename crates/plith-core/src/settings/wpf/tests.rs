use super::*;
use crate::theme::colour::Rgb;
use crate::theme::registry::{AccentPick, ThemeId};

#[test]
fn berkes_config_maps_to_gece_weather_on_no_city() {
    let text = "[General]\nTheme = Dark\n\n[Osd]\nShowWeather = True\nWeatherLocation = \n";
    let (settings, taken) = import(text);
    assert_eq!(settings.theme, ThemeId::gece());
    assert!(settings.weather);
    assert_eq!(settings.location, Settings::default().location);
    assert_eq!(settings.city, "");
    assert_eq!(taken, 2);
}

#[test]
fn light_imports_as_kagit_colours() {
    let (settings, taken) = import("[General]\nTheme = Light\n");
    let gece = ThemeId::gece();
    assert_eq!(settings.theme, gece);
    assert_eq!(settings.floors.get(&gece), Some(&Rgb([0xF3, 0xF2, 0xED])));
    assert_eq!(
        settings.accents.get(&gece),
        Some(&AccentPick::Colour(Rgb([0x3F, 0x6E, 0x00])))
    );
    assert_eq!(taken, 1);
}

#[test]
fn auto_theme_keeps_the_default() {
    let (settings, taken) = import("[General]\nTheme = Auto\n");
    assert_eq!(settings.theme, ThemeId::gece());
    assert!(settings.floors.is_empty() && settings.accents.is_empty());
    assert_eq!(taken, 1);
}

#[test]
fn weather_off_is_taken() {
    let (settings, _) = import("[Osd]\nShowWeather = false\n");
    assert!(!settings.weather);
}

#[test]
fn city_sets_city_mode() {
    let (settings, taken) = import("[Osd]\nWeatherLocation = Ankara\n");
    assert_eq!(settings.location, LocationMode::City);
    assert_eq!(settings.city, "Ankara");
    assert_eq!(taken, 1);
}

#[test]
fn a_long_city_is_capped() {
    let text = format!("[Osd]\nWeatherLocation = {}\n", "a".repeat(500));
    let (settings, _) = import(&text);
    assert_eq!(settings.city.chars().count(), 200);
}

#[test]
fn bom_and_crlf_tolerated() {
    let text = "\u{feff}[General]\r\nTheme = Light\r\n[Osd]\r\nShowWeather = False\r\n";
    let (settings, taken) = import(text);
    assert_eq!(settings.floors.len(), 1);
    assert_eq!(settings.theme, ThemeId::gece());
    assert!(!settings.weather);
    assert_eq!(taken, 2);
}

#[test]
fn keys_outside_their_section_and_unknown_keys_are_ignored() {
    let text =
        "Theme = Light\n[Osd]\nTheme = Light\nSomething = 1\n[General]\nShowWeather = False\n";
    let (settings, taken) = import(text);
    assert_eq!(settings, Settings::default());
    assert_eq!(taken, 0);
}

#[test]
fn empty_text_gives_defaults() {
    assert_eq!(import(""), (Settings::default(), 0));
}
