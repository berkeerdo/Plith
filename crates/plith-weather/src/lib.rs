//! Weather for the welcome view: an `Open-Meteo` hourly forecast fetched over `WinHTTP` by a
//! background worker. Parsing lives in `plith-core`; this crate does HTTP, location and timing.

mod fetch;
mod http;
mod location;
mod service;
mod urls;

pub use fetch::{Fetch, HttpFetch};
pub use service::WeatherService;

use plith_core::settings::LocationMode;

/// What the worker needs to find a forecast.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeatherConfig {
    /// Where the location comes from.
    pub location: LocationMode,
    /// The city for `LocationMode::City`, free text.
    pub city: String,
}
