//! The seam between the worker and the network: tests swap in a fake.

use crate::{WeatherConfig, http, location, urls};
use plith_core::weather::{Forecast, Stamp, parse_forecast};
use windows::Win32::System::SystemInformation::GetLocalTime;

/// Produces one forecast for a configuration.
pub trait Fetch: Send + Sync {
    /// Resolves the location and reads the forecast.
    ///
    /// # Errors
    ///
    /// Returns a short message when any step fails.
    fn fetch(&self, config: &WeatherConfig) -> Result<Forecast, String>;
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
    fn fetch(&self, config: &WeatherConfig) -> Result<Forecast, String> {
        let (latitude, longitude) = location::resolve(config)?;
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
