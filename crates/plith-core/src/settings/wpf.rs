//! A one-time read of the WPF app's `config.ini`, so a 0.3.x user keeps the theme and the
//! weather choice. Only the keys that still mean something are taken; everything else is ignored.

use super::{LocationMode, Settings};

/// The longest city taken from the old file, in characters.
const CITY_LIMIT: usize = 200;

/// Settings from the text of a WPF `config.ini`, and how many keys were taken. Starts from the
/// defaults. Reads `[General] Theme` (`Dark`, `Light` or `Auto`), `[Osd] ShowWeather`
/// (`True` or `False`) and `[Osd] WeatherLocation` (a non-empty value turns on city mode).
/// Sections, keys and values are matched without regard to case; a byte order mark and CRLF line
/// ends are fine; a later line wins over an earlier one; unknown keys are ignored.
#[must_use]
pub fn import(config_ini: &str) -> (Settings, usize) {
    let mut settings = Settings::default();
    let mut light = None;
    let mut weather = None;
    let mut city = None;
    let mut section = String::new();
    let text = config_ini.strip_prefix('\u{feff}').unwrap_or(config_ini);
    for raw in text.lines() {
        let line = raw.trim();
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            section = name.trim().to_ascii_lowercase();
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let (key, value) = (key.trim().to_ascii_lowercase(), value.trim());
        match (section.as_str(), key.as_str()) {
            ("general", "theme") => light = parse_theme(value).or(light),
            ("osd", "showweather") => weather = parse_bool(value).or(weather),
            ("osd", "weatherlocation") => city = Some(clean_city(value)),
            _ => {}
        }
    }
    let mut taken = 0;
    if let Some(light) = light {
        if light {
            super::migrate::kagit(&mut settings);
        }
        taken += 1;
    }
    if let Some(on) = weather {
        settings.weather = on;
        taken += 1;
    }
    if let Some(city) = city.filter(|c| !c.is_empty()) {
        settings.city = city;
        settings.location = LocationMode::City;
        taken += 1;
    }
    (settings, taken)
}

/// Whether the old theme was light; `Dark` and `Auto` are Gece as it is.
fn parse_theme(value: &str) -> Option<bool> {
    match value.to_ascii_lowercase().as_str() {
        "dark" | "auto" => Some(false),
        "light" => Some(true),
        _ => None,
    }
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.to_ascii_lowercase().as_str() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

/// The city is user text: no line breaks, at most `CITY_LIMIT` characters.
fn clean_city(value: &str) -> String {
    let no_breaks = value.replace(['\r', '\n'], "");
    no_breaks.trim().chars().take(CITY_LIMIT).collect()
}

#[cfg(test)]
mod tests;
