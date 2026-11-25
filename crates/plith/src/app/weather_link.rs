//! The weather service as the feed sees it: started only when weather is on in a normal run,
//! read without blocking, reconfigured or stopped off the UI thread while running, dropped
//! (never joined) at exit.

use plith_core::settings::{LocationMode, Settings};
use plith_core::weather::Forecast;
use plith_host::log::LogFile;
use plith_weather::{Hooks, WeatherConfig, WeatherService};
use std::sync::Arc;
use std::sync::mpsc::Receiver;

/// A running weather service and the forecasts it publishes.
pub struct WeatherLink {
    /// Dropping it asks the worker to stop without waiting for a fetch in flight; `stop()` would
    /// join the worker, which may be inside a fetch for tens of seconds, so it is only called on
    /// a thread of its own.
    service: Option<WeatherService>,
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

/// What the worker needs from `settings`.
fn config_of(settings: &Settings) -> WeatherConfig {
    WeatherConfig {
        location: settings.location,
        city: settings.city.clone(),
    }
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
    let (service, feed) = WeatherService::start(config_of(settings), hooks);
    Some(WeatherLink {
        service: Some(service),
        feed,
    })
}

impl WeatherLink {
    /// The newest forecast that arrived since the last call, if any. The channel is ordered, so
    /// an older forecast never follows a newer one.
    pub fn take(&mut self) -> Option<Forecast> {
        self.feed.try_iter().last()
    }

    /// Hands the worker a new location; it fetches once at once. Never blocks.
    pub fn reconfigure(&self, config: WeatherConfig) {
        if let Some(service) = &self.service {
            service.reconfigure(config);
        }
    }

    /// Stops the worker on a short thread of its own, which waits for a fetch in flight, so the
    /// caller never does. If that thread cannot start, the drop here still tells the worker to
    /// stop.
    pub fn stop_detached(self) {
        let Some(service) = self.service else {
            return;
        };
        let _ = std::thread::Builder::new()
            .name("plith-weather-stop".to_string())
            .spawn(move || service.stop());
    }

    /// A link over a running service, for tests.
    #[cfg(test)]
    pub fn from_service(service: WeatherService, feed: Receiver<Forecast>) -> Self {
        Self {
            service: Some(service),
            feed,
        }
    }

    /// A link over a bare channel, for tests.
    #[cfg(test)]
    pub fn from_feed(feed: Receiver<Forecast>) -> Self {
        Self {
            service: None,
            feed,
        }
    }
}

/// Posts a wake to the UI thread from the weather worker.
pub type Wake = Arc<dyn Fn() + Send + Sync>;

/// What starting a link needs besides the settings, kept by the app for the whole run.
pub struct WeatherWiring {
    log: Arc<LogFile>,
    waker: Wake,
    /// True in a normal run; probing runs never touch the network.
    network: bool,
}

impl WeatherWiring {
    /// The wiring of a run.
    #[must_use]
    pub fn new(log: Arc<LogFile>, waker: Wake, network: bool) -> Self {
        Self {
            log,
            waker,
            network,
        }
    }

    /// A link for `settings`, or `None` when weather is off or the run has no network.
    #[must_use]
    pub fn start(&self, settings: &Settings) -> Option<WeatherLink> {
        let waker = Arc::clone(&self.waker);
        start(settings, self.network, &self.log, Box::new(move || waker()))
    }

    /// Brings `link` in line with `settings`: on and running reconfigures it, off stops it off
    /// the UI thread, on and not running starts one. Returns the link the feed reads from.
    #[must_use]
    pub fn apply(&self, link: Option<WeatherLink>, settings: &Settings) -> Option<WeatherLink> {
        let on = settings.weather && self.network;
        match link {
            Some(mut link) if on => {
                // A forecast of the old place still queued is dropped; the new one follows.
                let _ = link.take();
                link.reconfigure(config_of(settings));
                Some(link)
            }
            Some(link) => {
                self.log.line("weather: off");
                link.stop_detached();
                None
            }
            None => self.start(settings),
        }
    }
}

#[cfg(test)]
mod tests;
