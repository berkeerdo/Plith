//! Where the forecast is for: the Windows location service, an IP lookup or a named city.

use crate::WeatherConfig;
use crate::{http, urls};
use plith_core::settings::LocationMode;
use plith_core::weather::{parse_geocode, parse_ip_location};
use windows::Devices::Geolocation::{GeolocationAccessStatus, Geolocator};
use windows::Foundation::TimeSpan;
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};

/// Finds the (latitude, longitude) for `config`. The Windows location service falls through to
/// the IP lookup when access is denied or no position arrives. Coordinates are never logged.
pub(crate) fn resolve(config: &WeatherConfig) -> Result<(f64, f64), String> {
    match config.location {
        LocationMode::Windows => windows_position().map_or_else(by_ip, Ok),
        LocationMode::Ip => by_ip(),
        LocationMode::City => by_city(&config.city),
    }
}

fn by_ip() -> Result<(f64, f64), String> {
    let body = http::get(urls::IP_URL)?;
    parse_ip_location(&body).ok_or_else(|| "unreadable ip location".to_string())
}

fn by_city(city: &str) -> Result<(f64, f64), String> {
    if city.trim().is_empty() {
        return Err("no city set".to_string());
    }
    let body = http::get(&urls::geocode_url(city.trim()))?;
    parse_geocode(&body)
        .map(|(latitude, longitude, _name)| (latitude, longitude))
        .ok_or_else(|| "city not found".to_string())
}

/// Joins the multithreaded apartment for this thread and leaves it on drop.
struct Apartment(bool);

impl Apartment {
    fn enter() -> Self {
        // SAFETY: a plain COM initialisation; balanced in `drop` when it succeeded.
        Self(unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok())
    }
}

impl Drop for Apartment {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: balances the successful `CoInitializeEx` in `enter`.
            unsafe { CoUninitialize() };
        }
    }
}

/// Seconds as `WinRT` 100 ns ticks.
fn seconds(n: i64) -> TimeSpan {
    TimeSpan {
        Duration: n * 10_000_000,
    }
}

/// The Windows position, only after an access check says it is allowed.
fn windows_position() -> Option<(f64, f64)> {
    let _apartment = Apartment::enter();
    let status = Geolocator::RequestAccessAsync().ok()?.join().ok()?;
    if status != GeolocationAccessStatus::Allowed {
        return None;
    }
    let position = Geolocator::new()
        .ok()?
        .GetGeopositionAsyncWithAgeAndTimeout(seconds(600), seconds(10))
        .ok()?
        .join()
        .ok()?;
    let point = position.Coordinate().ok()?.Point().ok()?.Position().ok()?;
    Some((point.Latitude, point.Longitude))
}
