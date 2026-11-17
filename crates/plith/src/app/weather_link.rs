//! The weather service as the feed sees it: started only when weather is on in a normal run,
//! read without blocking, dropped (never joined) at exit.

use crate::log::LogFile;
use plith_core::settings::{LocationMode, Settings};
use plith_core::weather::Forecast;
use plith_weather::{Hooks, WeatherConfig, WeatherService};
use std::sync::Arc;
use std::sync::mpsc::Receiver;

/// A running weather service and the forecasts it publishes.
pub struct WeatherLink {
    /// Dropping it asks the worker to stop without waiting for a fetch in flight; `stop()` would
    /// join the worker, which may be inside a fetch for tens of seconds, so it is never called
    /// on the UI thread.
    _service: Option<WeatherService>,
    /// Subscribed before the worker starts, so it carries every forecast, oldest first.
    feed: Receiver<Forecast>,
}

/// The one log line of a weather start: the mode, never the city, the coordinates or the IP.
#[must_use]
pub fn start_line(settings: &Settings) -> String {
    let mode = match settings.location {
        LocationMode::Windows => "windows",
        LocationMode::Ip => "ip",
        LocationMode::City => "city",
    };
    format!("weather: on, location: {mode}")
}

/// Starts the service when weather is on and `network` allows it (a normal run: demo, fixture
/// and idle-check runs never touch the network). `waker` runs on the worker thread after each
/// forecast; it must only post a wake and touch no app state, as the window may be gone. A failed
/// fetch is logged by the service's own line, which holds no coordinates, city, IP or URL.
#[must_use]
pub fn start(
    settings: &Settings,
    network: bool,
    log: &Arc<LogFile>,
    waker: Box<dyn Fn() + Send>,
) -> Option<WeatherLink> {
    if !settings.weather || !network {
        return None;
    }
    log.line(&start_line(settings));
    let failure_log = Arc::clone(log);
    let hooks = Hooks {
        waker,
        on_failure: Box::new(move |line| failure_log.line(line)),
    };
    let (service, feed) = WeatherService::start(
        WeatherConfig {
            location: settings.location,
            city: settings.city.clone(),
        },
        hooks,
    );
    Some(WeatherLink {
        _service: Some(service),
        feed,
    })
}

impl WeatherLink {
    /// The newest forecast that arrived since the last call, if any. The channel is ordered, so
    /// an older forecast never follows a newer one.
    pub fn take(&mut self) -> Option<Forecast> {
        self.feed.try_iter().last()
    }

    /// A link over a bare channel, for tests.
    #[cfg(test)]
    pub fn from_feed(feed: Receiver<Forecast>) -> Self {
        Self {
            _service: None,
            feed,
        }
    }
}

#[cfg(test)]
mod tests;
