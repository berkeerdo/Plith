//! The background weather worker: fetch at start, every 30 minutes, after a reconfigure and
//! after a failure on a short backoff.

use crate::{Fetch, HttpFetch, WeatherConfig};
use plith_core::weather::Forecast;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::Duration;

/// How long the worker sleeps between successful fetches.
const REFRESH: Duration = Duration::from_mins(30);

/// How long the worker waits after the first, second and third failure in a row; after that it
/// is back to `REFRESH`.
const RETRY: [Duration; 3] = [
    Duration::from_mins(2),
    Duration::from_mins(5),
    Duration::from_mins(10),
];

/// Runs on the worker thread after each new forecast.
pub type Waker = Box<dyn Fn() + Send>;

/// Runs on the worker thread after a failed fetch with a line safe to log: the failed step and
/// the retry delay, never a coordinate, city, IP or URL.
pub type FailureLog = Box<dyn Fn(&str) + Send>;

/// The callbacks the worker needs from its first fetch on, so none can be missed.
pub struct Hooks {
    /// Run after each new forecast; it must only post a wake and touch no app state.
    pub waker: Waker,
    /// Run after each failed fetch with a loggable line.
    pub on_failure: FailureLog,
}

/// The worker's waits, replaceable so tests need not sleep for minutes.
pub(crate) struct Timing {
    pub(crate) refresh: Duration,
    pub(crate) retry: Vec<Duration>,
}

impl Timing {
    fn real() -> Self {
        Self {
            refresh: REFRESH,
            retry: RETRY.to_vec(),
        }
    }

    /// The wait after the `failures`-th failure in a row (1 for the first).
    fn after_failure(&self, failures: usize) -> Duration {
        failures
            .checked_sub(1)
            .and_then(|i| self.retry.get(i))
            .copied()
            .unwrap_or(self.refresh)
    }
}

enum Command {
    Reconfigure(WeatherConfig),
    Stop,
}

struct Shared {
    current: Mutex<Option<Forecast>>,
    subscribers: Mutex<Vec<Sender<Forecast>>>,
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Fetches the forecast in the background and publishes every new one.
pub struct WeatherService {
    shared: Arc<Shared>,
    commands: Sender<Command>,
    worker: Option<JoinHandle<()>>,
}

impl WeatherService {
    /// Starts the worker over the real network. Returns at once; the worker does the fetching.
    /// The receiver gets every forecast from the very first one, so nothing is missed.
    #[must_use]
    pub fn start(config: WeatherConfig, hooks: Hooks) -> (Self, Receiver<Forecast>) {
        Self::start_with(config, Arc::new(HttpFetch), hooks, Timing::real())
    }

    /// Starts the worker over any `Fetch`.
    pub(crate) fn start_with(
        config: WeatherConfig,
        fetcher: Arc<dyn Fetch>,
        hooks: Hooks,
        timing: Timing,
    ) -> (Self, Receiver<Forecast>) {
        let (tx, feed) = channel();
        let shared = Arc::new(Shared {
            current: Mutex::new(None),
            subscribers: Mutex::new(vec![tx]),
        });
        let (commands, inbox) = channel();
        let worker = {
            let shared = Arc::clone(&shared);
            std::thread::Builder::new()
                .name("plith-weather".to_string())
                .spawn(move || {
                    let job = Job {
                        fetcher: fetcher.as_ref(),
                        hooks: &hooks,
                        timing: &timing,
                    };
                    run(&shared, &inbox, &job, config);
                })
                .ok()
        };
        let service = Self {
            shared,
            commands,
            worker,
        };
        (service, feed)
    }

    /// The latest forecast, if one has been read.
    #[must_use]
    pub fn snapshot(&self) -> Option<Forecast> {
        lock(&self.shared.current).clone()
    }

    /// Replaces the configuration and fetches once at once.
    pub fn reconfigure(&self, config: WeatherConfig) {
        let _ = self.commands.send(Command::Reconfigure(config));
    }

    /// Stops the worker and waits for it. A fetch in flight finishes first (10 s at most per step).
    pub fn stop(mut self) {
        let _ = self.commands.send(Command::Stop);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for WeatherService {
    fn drop(&mut self) {
        let _ = self.commands.send(Command::Stop);
    }
}

/// Where, once per configuration, then the forecast.
fn attempt(
    fetcher: &dyn Fetch,
    config: &WeatherConfig,
    point: &mut Option<(f64, f64)>,
) -> Result<Forecast, String> {
    if point.is_none() {
        *point = Some(fetcher.locate(config)?);
    }
    match *point {
        Some(at) => fetcher.forecast(at),
        None => Err("no position".to_string()),
    }
}

/// What the worker thread owns besides the shared state.
struct Job<'a> {
    fetcher: &'a dyn Fetch,
    hooks: &'a Hooks,
    timing: &'a Timing,
}

fn run(shared: &Shared, inbox: &Receiver<Command>, job: &Job, mut config: WeatherConfig) {
    let (fetcher, timing) = (job.fetcher, job.timing);
    let mut point = None;
    let mut failures = 0;
    loop {
        let wait = match attempt(fetcher, &config, &mut point) {
            Ok(forecast) => {
                failures = 0;
                publish(shared, &forecast);
                (job.hooks.waker)();
                timing.refresh
            }
            Err(reason) => {
                failures += 1;
                let delay = timing.after_failure(failures);
                (job.hooks.on_failure)(&failure_line(&reason, delay));
                delay
            }
        };
        match inbox.recv_timeout(wait) {
            Err(RecvTimeoutError::Timeout) => {}
            Ok(Command::Reconfigure(next)) => {
                config = next;
                point = None;
                failures = 0;
            }
            Ok(Command::Stop) | Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

fn failure_line(reason: &str, delay: Duration) -> String {
    let secs = delay.as_secs();
    if secs >= 60 {
        format!(
            "weather: fetch failed ({reason}), retry in {} min",
            secs / 60
        )
    } else {
        format!("weather: fetch failed ({reason}), retry in {secs} s")
    }
}

fn publish(shared: &Shared, forecast: &Forecast) {
    *lock(&shared.current) = Some(forecast.clone());
    lock(&shared.subscribers).retain(|tx| tx.send(forecast.clone()).is_ok());
}

#[cfg(test)]
mod tests;
