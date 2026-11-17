//! The background weather worker: fetch at start, every 30 minutes and after a reconfigure.

use crate::{Fetch, HttpFetch, WeatherConfig};
use plith_core::weather::Forecast;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::Duration;

/// How long the worker sleeps between fetches.
const REFRESH: Duration = Duration::from_mins(30);

type Waker = Box<dyn Fn() + Send>;

enum Command {
    Reconfigure(WeatherConfig),
    Stop,
}

#[derive(Default)]
struct Shared {
    current: Mutex<Option<Forecast>>,
    subscribers: Mutex<Vec<Sender<Forecast>>>,
    waker: Mutex<Option<Waker>>,
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
    #[must_use]
    pub fn start(config: WeatherConfig) -> Self {
        Self::start_with(config, Arc::new(HttpFetch))
    }

    /// Starts the worker over any `Fetch`.
    pub(crate) fn start_with(config: WeatherConfig, fetcher: Arc<dyn Fetch>) -> Self {
        let shared = Arc::new(Shared::default());
        let (commands, inbox) = channel();
        let worker = {
            let shared = Arc::clone(&shared);
            std::thread::Builder::new()
                .name("plith-weather".to_string())
                .spawn(move || run(&shared, &inbox, fetcher.as_ref(), config))
                .ok()
        };
        Self {
            shared,
            commands,
            worker,
        }
    }

    /// The latest forecast, if one has been read.
    #[must_use]
    pub fn snapshot(&self) -> Option<Forecast> {
        lock(&self.shared.current).clone()
    }

    /// A channel that receives every new forecast from now on.
    #[must_use]
    pub fn subscribe(&self) -> Receiver<Forecast> {
        let (tx, rx) = channel();
        lock(&self.shared.subscribers).push(tx);
        rx
    }

    /// Sets the callback run after each new forecast. It runs on the worker thread.
    pub fn set_waker(&self, waker: Waker) {
        *lock(&self.shared.waker) = Some(waker);
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

fn run(shared: &Shared, inbox: &Receiver<Command>, fetcher: &dyn Fetch, mut config: WeatherConfig) {
    loop {
        if let Ok(forecast) = fetcher.fetch(&config) {
            publish(shared, &forecast);
        }
        match inbox.recv_timeout(REFRESH) {
            Err(RecvTimeoutError::Timeout) => {}
            Ok(Command::Reconfigure(next)) => config = next,
            Ok(Command::Stop) | Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

fn publish(shared: &Shared, forecast: &Forecast) {
    *lock(&shared.current) = Some(forecast.clone());
    lock(&shared.subscribers).retain(|tx| tx.send(forecast.clone()).is_ok());
    if let Some(waker) = lock(&shared.waker).as_ref() {
        waker();
    }
}

#[cfg(test)]
mod tests;
