use super::*;
use plith_core::settings::{LocationMode, Settings};
use std::sync::Mutex;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

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

/// Reports every `locate` and fails it after `delay`, so no forecast ever arrives.
struct FakeFetch {
    locates: Mutex<Sender<WeatherConfig>>,
    delay: Duration,
}

impl plith_weather::Fetch for FakeFetch {
    fn locate(&self, config: &WeatherConfig) -> Result<(f64, f64), String> {
        let _ = self.locates.lock().unwrap().send(config.clone());
        std::thread::sleep(self.delay);
        Err("offline".to_string())
    }

    fn forecast(&self, _point: (f64, f64)) -> Result<Forecast, String> {
        Err("offline".to_string())
    }
}

const WAIT: Duration = Duration::from_secs(5);

fn city(name: &str) -> WeatherConfig {
    WeatherConfig {
        location: LocationMode::City,
        city: name.to_string(),
    }
}

/// A link over a real service whose network is `FakeFetch`, and the channel of its locates.
fn faked(delay: Duration) -> (WeatherLink, Receiver<WeatherConfig>) {
    let (tx, locates) = mpsc::channel();
    let fetch = Arc::new(FakeFetch {
        locates: Mutex::new(tx),
        delay,
    });
    let hooks = Hooks {
        waker: Box::new(|| {}),
        on_failure: Box::new(|_| {}),
    };
    let (service, feed) = WeatherService::start_over(city("Alpha"), fetch, hooks);
    (WeatherLink::from_service(service, feed), locates)
}

#[test]
fn weather_reconfigure_reaches_service() {
    let (link, locates) = faked(Duration::ZERO);
    assert_eq!(locates.recv_timeout(WAIT).unwrap(), city("Alpha"));
    link.reconfigure(city("Beta"));
    assert_eq!(locates.recv_timeout(WAIT).unwrap(), city("Beta"));
}

fn wiring(network: bool) -> WeatherWiring {
    WeatherWiring::new(log("wiring"), Arc::new(|| {}), network)
}

fn on_in(name: &str) -> Settings {
    Settings {
        city: name.to_string(),
        ..on()
    }
}

#[test]
fn a_location_change_while_on_reconfigures_the_running_link() {
    let (link, locates) = faked(Duration::ZERO);
    assert_eq!(locates.recv_timeout(WAIT).unwrap(), city("Alpha"));
    let link = wiring(true).apply(Some(link), &on_in("Beta"));
    assert!(link.is_some(), "the same link stays");
    assert_eq!(locates.recv_timeout(WAIT).unwrap(), city("Beta"));
}

#[test]
fn weather_off_stops_the_service_without_waiting() {
    // The worker sits in a slow locate when weather goes off.
    let (link, locates) = faked(Duration::from_millis(400));
    assert_eq!(locates.recv_timeout(WAIT).unwrap(), city("Alpha"));
    let begun = Instant::now();
    let link = wiring(true).apply(Some(link), &Settings::default());
    let took = begun.elapsed();
    assert!(link.is_none());
    assert!(took < Duration::from_millis(100), "off took {took:?}");
    // The worker ends (its fetcher, and the sender in it, drops) once the locate returns.
    assert_eq!(
        locates.recv_timeout(WAIT),
        Err(RecvTimeoutError::Disconnected)
    );
}

#[test]
fn weather_on_in_a_probing_run_starts_nothing() {
    assert!(wiring(false).apply(None, &on()).is_none());
}
