use super::*;
use plith_core::settings::{LocationMode, Settings};

fn log(name: &str) -> Arc<LogFile> {
    let dir = std::env::temp_dir().join(format!("plith-weather-{name}-{}", std::process::id()));
    Arc::new(LogFile::open_at(&dir.join("plith.log")).unwrap())
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

fn forecast(minute: u8) -> Forecast {
    Forecast {
        hours: Vec::new(),
        read_at: plith_core::weather::Stamp {
            day: (2026, 11, 17),
            hour: 9,
            minute,
        },
    }
}

#[test]
fn take_never_hands_back_an_older_forecast() {
    let (tx, rx) = std::sync::mpsc::channel();
    let mut link = WeatherLink::from_feed(rx);
    assert_eq!(link.take(), None);
    tx.send(forecast(1)).unwrap();
    tx.send(forecast(2)).unwrap();
    assert_eq!(link.take(), Some(forecast(2)));
    // Nothing new: the answer is nothing, not the earlier forecast.
    assert_eq!(link.take(), None);
    tx.send(forecast(3)).unwrap();
    assert_eq!(link.take(), Some(forecast(3)));
    assert_eq!(link.take(), None);
}
