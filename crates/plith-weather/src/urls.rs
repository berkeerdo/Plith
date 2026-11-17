//! The request URLs. Every one is HTTPS.

use std::fmt::Write;

/// Where the public IP lookup lives.
pub(crate) const IP_URL: &str = "https://ipwho.is/";

/// The hourly forecast for a point, in the point's own time zone.
pub(crate) fn forecast_url(latitude: f64, longitude: f64) -> String {
    format!(
        "https://api.open-meteo.com/v1/forecast?latitude={latitude:.4}&longitude={longitude:.4}\
         &hourly=temperature_2m,weather_code,precipitation_probability\
         &timezone=auto&forecast_days=2"
    )
}

/// The geocoding search for a city name, first result only.
pub(crate) fn geocode_url(city: &str) -> String {
    format!(
        "https://geocoding-api.open-meteo.com/v1/search?name={}&count=1&format=json",
        percent_encode(city)
    )
}

/// Percent-encodes every UTF-8 byte that is not an unreserved URL character.
fn percent_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for &byte in text.as_bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(byte));
        } else {
            let _ = write!(out, "%{byte:02X}");
        }
    }
    out
}

#[cfg(test)]
mod tests;
