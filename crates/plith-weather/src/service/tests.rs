use super::*;
use plith_core::settings::LocationMode;
use plith_core::weather::{Forecast, Stamp};
use std::collections::VecDeque;
use std::sync::mpsc::{Receiver, Sender, channel};

const WAIT: Duration = Duration::from_secs(5);

fn config(city: &str) -> WeatherConfig {
    WeatherConfig {
        location: LocationMode::City,
        city: city.to_string(),
    }
}

fn forecast(minute: u8) -> Forecast {
    Forecast {
        hours: Vec::new(),
        read_at: Stamp {
            day: (2026, 11, 17),
            hour: 9,
            minute,
        },
    }
}

/// Answers forecasts from a queue and reports every `locate` call on a channel.
struct Fake {
    results: Mutex<VecDeque<Result<Forecast, String>>>,
    locates: Mutex<Sender<WeatherConfig>>,
    locate_error: Mutex<Option<String>>,
}

impl Fetch for Fake {
    fn locate(&self, config: &WeatherConfig) -> Result<(f64, f64), String> {
        self.locates.lock().unwrap().send(config.clone()).unwrap();
        match self.locate_error.lock().unwrap().take() {
            Some(reason) => Err(reason),
            None => Ok((1.0, 2.0)),
        }
    }

    fn forecast(&self, _point: (f64, f64)) -> Result<Forecast, String> {
        self.results
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| Err("queue empty".to_string()))
    }
}

/// A started service with a channel for every signal it gives. Nothing here sleeps: the tests
/// wait on those channels, and `stop()` joins the worker so a final state is settled.
struct Rig {
    service: WeatherService,
    updates: Receiver<Forecast>,
    wakes: Receiver<()>,
    failures: Receiver<String>,
    locates: Receiver<WeatherConfig>,
}

fn fast_retry() -> Timing {
    Timing {
        refresh: Duration::from_secs(3600),
        retry: vec![Duration::from_millis(5), Duration::from_millis(5)],
    }
}

fn rig(results: Vec<Result<Forecast, String>>, locate_error: Option<&str>) -> Rig {
    let (locate_tx, locates) = channel();
    let fake = Fake {
        results: Mutex::new(results.into()),
        locates: Mutex::new(locate_tx),
        locate_error: Mutex::new(locate_error.map(str::to_string)),
    };
    let (wake_tx, wakes) = channel();
    let (failure_tx, failures) = channel();
    let hooks = Hooks {
        waker: Box::new(move || {
            let _ = wake_tx.send(());
        }),
        on_failure: Box::new(move |line| {
            let _ = failure_tx.send(line.to_string());
        }),
    };
    let (service, updates) =
        WeatherService::start_with(config("a"), Arc::new(fake), hooks, fast_retry());
    Rig {
        service,
        updates,
        wakes,
        failures,
        locates,
    }
}

#[test]
fn failed_fetch_keeps_last_forecast() {
    let rig = rig(vec![Ok(forecast(1)), Err("offline".to_string())], None);
    assert_eq!(rig.updates.recv_timeout(WAIT), Ok(forecast(1)));

    rig.service.reconfigure(config("b"));
    rig.failures.recv_timeout(WAIT).expect("the failure");
    assert_eq!(rig.service.snapshot(), Some(forecast(1)));
    rig.service.stop();
    // The failed fetch published nothing.
    assert!(rig.updates.try_recv().is_err());
}

#[test]
fn reconfigure_triggers_one_fetch() {
    let rig = rig(vec![Ok(forecast(1)), Ok(forecast(2))], None);
    assert_eq!(rig.locates.recv_timeout(WAIT), Ok(config("a")));
    assert_eq!(rig.updates.recv_timeout(WAIT), Ok(forecast(1)));

    rig.service.reconfigure(config("b"));
    assert_eq!(rig.locates.recv_timeout(WAIT), Ok(config("b")));
    assert_eq!(rig.updates.recv_timeout(WAIT), Ok(forecast(2)));
    rig.service.stop();
    assert!(rig.locates.try_recv().is_err());
}

#[test]
fn waker_fires_once_per_new_forecast() {
    let rig = rig(vec![Ok(forecast(1))], None);
    assert_eq!(rig.wakes.recv_timeout(WAIT), Ok(()));
    rig.service.stop();
    // The worker is joined, so a second wake could not still be on its way.
    assert!(rig.wakes.try_recv().is_err());
}

#[test]
fn a_failed_fetch_retries_and_logs_without_details() {
    let rig = rig(
        vec![Err("http status 503".to_string()), Ok(forecast(3))],
        None,
    );
    let line = rig.failures.recv_timeout(WAIT).expect("the failure line");
    assert!(line.starts_with("weather: fetch failed (http status 503), retry in "));
    assert!(!line.contains("latitude") && !line.contains('?') && !line.contains("://"));
    assert_eq!(rig.updates.recv_timeout(WAIT), Ok(forecast(3)));
    rig.service.stop();
}

#[test]
fn the_position_is_looked_up_once_across_retries() {
    let results = vec![
        Err("offline".to_string()),
        Err("offline".to_string()),
        Ok(forecast(1)),
    ];
    let rig = rig(results, None);
    assert_eq!(rig.updates.recv_timeout(WAIT), Ok(forecast(1)));
    rig.service.stop();
    assert_eq!(rig.locates.try_iter().count(), 1);
}

#[test]
fn a_failed_location_is_retried() {
    let rig = rig(vec![Ok(forecast(1))], Some("no city set"));
    let line = rig.failures.recv_timeout(WAIT).expect("the failure line");
    assert!(line.contains("(no city set)"));
    assert_eq!(rig.updates.recv_timeout(WAIT), Ok(forecast(1)));
    rig.service.stop();
    assert_eq!(rig.locates.try_iter().count(), 2);
}

#[test]
fn a_reconfigure_looks_the_position_up_again() {
    let rig = rig(vec![Ok(forecast(1)), Ok(forecast(2))], None);
    assert_eq!(rig.updates.recv_timeout(WAIT), Ok(forecast(1)));
    rig.service.reconfigure(config("b"));
    assert_eq!(rig.updates.recv_timeout(WAIT), Ok(forecast(2)));
    rig.service.stop();
    assert_eq!(rig.locates.try_iter().count(), 2);
}

#[test]
fn the_real_backoff_is_two_five_ten_then_thirty_minutes() {
    let timing = Timing::real();
    let minutes = |n| timing.after_failure(n).as_secs() / 60;
    assert_eq!(
        [minutes(1), minutes(2), minutes(3), minutes(4)],
        [2, 5, 10, 30]
    );
    assert_eq!(minutes(40), 30);
    assert_eq!(
        failure_line("offline", Duration::from_mins(5)),
        "weather: fetch failed (offline), retry in 5 min"
    );
}
