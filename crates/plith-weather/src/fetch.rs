//! The seam between the worker and the network: tests swap in a fake.

use crate::{WeatherConfig, http, location, urls};
use plith_core::weather::{Forecast, Stamp, parse_forecast};
use windows::Win32::System::SystemInformation::GetLocalTime;

/// The two steps of a forecast: where, then what. They are separate so the worker can keep the
/// position and ask the location source (a third party for IP lookups) once per configuration.
pub trait Fetch: Send + Sync {
    /// Finds the (latitude, longitude) for a configuration. Never logged.
    ///
    /// # Errors
    ///
    /// Returns a short message, free of coordinates and URLs, when the lookup fails.
    fn locate(&self, config: &WeatherConfig) -> Result<(f64, f64), String>;

    /// Reads the forecast for a point.
    ///
    /// # Errors
    ///
    /// Returns a short message, free of coordinates and URLs, when the read fails.
    fn forecast(&self, point: (f64, f64)) -> Result<Forecast, String>;
}

/// The real thing: location, then `Open-Meteo`, over `WinHTTP`.
#[derive(Debug, Default, Clone, Copy)]
pub struct HttpFetch;

impl HttpFetch {
    /// Fetches the forecast for a point, with no location lookup.
    ///
    /// # Errors
    ///
    /// Returns a short message when the request or the parsing fails.
    pub fn forecast_at(latitude: f64, longitude: f64) -> Result<Forecast, String> {
        let body = http::get(&urls::forecast_url(latitude, longitude))?;
        parse_forecast(&body, local_stamp())
    }
}

impl Fetch for HttpFetch {
    fn locate(&self, config: &WeatherConfig) -> Result<(f64, f64), String> {
        location::resolve(config)
    }

    fn forecast(&self, (latitude, longitude): (f64, f64)) -> Result<Forecast, String> {
        Self::forecast_at(latitude, longitude)
    }
}

/// The PC's local time to the minute.
fn local_stamp() -> Stamp {
    // SAFETY: `GetLocalTime` has no preconditions.
    let now = unsafe { GetLocalTime() };
    Stamp {
        day: (now.wYear, now.wMonth, now.wDay),
        hour: u8::try_from(now.wHour).unwrap_or(0),
        minute: u8::try_from(now.wMinute).unwrap_or(0),
    }
}
