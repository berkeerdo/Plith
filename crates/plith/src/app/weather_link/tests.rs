use super::*;
use plith_core::settings::{LocationMode, Settings};

fn log(name: &str) -> LogFile {
    let dir = std::env::temp_dir().join(format!("plith-weather-{name}-{}", std::process::id()));
    LogFile::open_at(&dir.join("plith.log")).unwrap()
}

fn on() -> Settings {
    Settings {
        weather: true,
        location: LocationMode::City,
        ..Settings::default()
    }
}

#[test]
fn weather_off_starts_no_weather_service() {
    let log = log("off");
    assert!(start(&Settings::default(), true, &log, Box::new(|| {})).is_none());
}

#[test]
fn probing_runs_start_no_weather_service_even_when_on() {
    let log = log("probe");
    assert!(start(&on(), false, &log, Box::new(|| {})).is_none());
}

#[test]
fn the_start_line_names_the_mode_and_nothing_else() {
    let mut settings = on();
    settings.city = "Secret Town".into();
    let line = start_line(&settings);
    assert_eq!(line, "weather: on, location: city");
    assert!(!line.contains("Secret"));
}
