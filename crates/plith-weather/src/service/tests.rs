use super::*;
use plith_core::settings::LocationMode;
use plith_core::weather::{Forecast, Stamp};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

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

/// Answers from a queue and reports every call on a channel.
struct Fake {
    results: Mutex<VecDeque<Result<Forecast, String>>>,
    calls: Mutex<Sender<WeatherConfig>>,
}

impl Fetch for Fake {
    fn fetch(&self, config: &WeatherConfig) -> Result<Forecast, String> {
        self.calls.lock().unwrap().send(config.clone()).unwrap();
        self.results
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| Err("queue empty".to_string()))
    }
}

fn start_fake(results: Vec<Result<Forecast, String>>) -> (WeatherService, Receiver<WeatherConfig>) {
    let (tx, calls) = channel();
    let fake = Fake {
        results: Mutex::new(results.into()),
        calls: Mutex::new(tx),
    };
    (
        WeatherService::start_with(config("a"), Arc::new(fake)),
        calls,
    )
}

fn next_call(calls: &Receiver<WeatherConfig>) -> WeatherConfig {
    calls.recv_timeout(Duration::from_secs(5)).expect("a fetch")
}

fn wait_for(what: &str, mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !done() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn failed_fetch_keeps_last_forecast() {
    let (service, calls) = start_fake(vec![Ok(forecast(1)), Err("offline".to_string())]);
    let updates = service.subscribe();
    next_call(&calls);
    wait_for("first forecast", || service.snapshot().is_some());

    service.reconfigure(config("b"));
    next_call(&calls);
    // A third call proves the failed second one is fully handled.
    service.reconfigure(config("c"));
    next_call(&calls);

    assert_eq!(service.snapshot(), Some(forecast(1)));
    assert_eq!(updates.try_iter().collect::<Vec<_>>(), [forecast(1)]);
    service.stop();
}

#[test]
fn reconfigure_triggers_one_fetch() {
    let (service, calls) = start_fake(vec![Ok(forecast(1)), Ok(forecast(2))]);
    assert_eq!(next_call(&calls), config("a"));

    service.reconfigure(config("b"));
    assert_eq!(next_call(&calls), config("b"));
    assert!(calls.recv_timeout(Duration::from_millis(300)).is_err());
    service.stop();
}

#[test]
fn waker_fires_once_per_new_forecast() {
    let count = Arc::new(AtomicUsize::new(0));
    let (service, calls) = start_fake(vec![Ok(forecast(1))]);
    let seen = Arc::clone(&count);
    service.set_waker(Box::new(move || {
        seen.fetch_add(1, Ordering::SeqCst);
    }));
    next_call(&calls);
    wait_for("wake", || count.load(Ordering::SeqCst) >= 1);
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(count.load(Ordering::SeqCst), 1);
    service.stop();
}
