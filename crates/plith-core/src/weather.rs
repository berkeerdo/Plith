//! The forecast model, Open-Meteo parsing and the small rules the welcome view and pill share.

pub use crate::json;

use crate::json::Json;
use crate::settings::TempUnit;

/// A calendar day as (year, month, day).
pub type Day = (u16, u16, u16);

/// A point in local time, to the minute.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Stamp {
    /// The calendar day.
    pub day: Day,
    /// The hour, 0 to 23.
    pub hour: u8,
    /// The minute, 0 to 59.
    pub minute: u8,
}

/// The four skies the island tells apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sky {
    /// Clear or mostly clear.
    Clear,
    /// Cloud, fog and anything unknown.
    Cloud,
    /// Drizzle, rain, showers and thunderstorms.
    Rain,
    /// Snow.
    Snow,
}

/// One forecast hour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hour {
    /// The day.
    pub day: Day,
    /// The hour, 0 to 23.
    pub hour: u8,
    /// The sky.
    pub sky: Sky,
    /// The temperature in Celsius.
    pub temp_c: f32,
    /// The chance of precipitation, when the source gave one.
    pub rain_pct: Option<u8>,
}

/// A forecast and when it was read.
#[derive(Debug, Clone, PartialEq)]
pub struct Forecast {
    /// The hours, in source order.
    pub hours: Vec<Hour>,
    /// When the forecast was fetched.
    pub read_at: Stamp,
}

/// Staleness limit in minutes.
const STALE_MINUTES: i64 = 90;

/// Maps a WMO weather code to a sky.
#[must_use]
pub fn sky_of(wmo: u16) -> Sky {
    match wmo {
        0 | 1 => Sky::Clear,
        71..=77 | 85 | 86 => Sky::Snow,
        51..=67 | 80..=82 | 95..=99 => Sky::Rain,
        _ => Sky::Cloud,
    }
}

/// Parses an Open-Meteo hourly forecast body.
///
/// # Errors
///
/// Returns a short message when the body is not JSON, a `hourly` array is missing or the arrays
/// differ in length, or a time, temperature or code is unreadable.
pub fn parse_forecast(body: &str, read_at: Stamp) -> Result<Forecast, String> {
    let root = json::parse(body)?;
    let hourly = root.get("hourly").ok_or("missing hourly")?;
    let array = |key: &str| {
        hourly
            .get(key)
            .and_then(Json::as_arr)
            .ok_or_else(|| format!("missing hourly.{key}"))
    };
    let times = array("time")?;
    let temps = array("temperature_2m")?;
    let codes = array("weather_code")?;
    let rains = array("precipitation_probability")?;
    if [temps.len(), codes.len(), rains.len()]
        .iter()
        .any(|&n| n != times.len())
    {
        return Err("hourly arrays differ in length".to_string());
    }
    let mut hours = Vec::with_capacity(times.len());
    for i in 0..times.len() {
        let (day, hour) = times[i]
            .as_str()
            .and_then(parse_time)
            .ok_or_else(|| format!("bad time at {i}"))?;
        let temp = temps[i]
            .as_f64()
            .ok_or_else(|| format!("bad temperature at {i}"))?;
        let code = codes[i]
            .as_f64()
            .ok_or_else(|| format!("bad weather code at {i}"))?;
        hours.push(Hour {
            day,
            hour,
            sky: sky_of(to_code(code)),
            temp_c: to_f32(temp),
            rain_pct: rains[i].as_f64().map(to_percent),
        });
    }
    Ok(Forecast { hours, read_at })
}

#[allow(clippy::cast_possible_truncation)]
fn to_f32(v: f64) -> f32 {
    v as f32
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn to_code(v: f64) -> u16 {
    v.clamp(0.0, f64::from(u16::MAX)) as u16
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn to_percent(v: f64) -> u8 {
    v.clamp(0.0, 100.0).round() as u8
}

/// Parses `YYYY-MM-DDTHH:MM`.
fn parse_time(text: &str) -> Option<(Day, u8)> {
    let b = text.as_bytes();
    if b.len() != 16 || b[4] != b'-' || b[7] != b'-' || b[10] != b'T' || b[13] != b':' {
        return None;
    }
    let num = |range: std::ops::Range<usize>| text.get(range)?.parse::<u16>().ok();
    let (year, month, day) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let (hour, minute) = (num(11..13)?, num(14..16)?);
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 || minute > 59 {
        return None;
    }
    Some(((year, month, day), u8::try_from(hour).ok()?))
}

/// Reads latitude and longitude from an IP location body.
#[must_use]
pub fn parse_ip_location(body: &str) -> Option<(f64, f64)> {
    let root = json::parse(body).ok()?;
    Some((
        root.get("latitude")?.as_f64()?,
        root.get("longitude")?.as_f64()?,
    ))
}

/// Reads latitude, longitude and name of the first geocoding result.
#[must_use]
pub fn parse_geocode(body: &str) -> Option<(f64, f64, String)> {
    let root = json::parse(body).ok()?;
    let first = root.get("results")?.as_arr()?.first()?;
    Some((
        first.get("latitude")?.as_f64()?,
        first.get("longitude")?.as_f64()?,
        first.get("name")?.as_str()?.to_string(),
    ))
}

/// The first later hour within `within_h` hours whose sky differs from the current hour's.
#[must_use]
pub fn next_turn(f: &Forecast, now: (Day, u8), within_h: u8) -> Option<(u8, Sky)> {
    let start = f.hours.iter().position(|h| (h.day, h.hour) == now)?;
    let current = f.hours[start].sky;
    f.hours
        .iter()
        .skip(start + 1)
        .take(usize::from(within_h))
        .find(|h| h.sky != current)
        .map(|h| (h.hour, h.sky))
}

/// Whether the forecast is more than 90 minutes old.
#[must_use]
pub fn is_stale(read_at: Stamp, now: Stamp) -> bool {
    minutes(now) - minutes(read_at) > STALE_MINUTES
}

fn minutes(s: Stamp) -> i64 {
    days_from_civil(s.day) * 1440 + i64::from(s.hour) * 60 + i64::from(s.minute)
}

/// Days since 1970-01-01 for a civil date (Howard Hinnant's algorithm).
fn days_from_civil((y, m, d): Day) -> i64 {
    let (y, m, d) = (i64::from(y), i64::from(m), i64::from(d));
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The temperature as shown, rounded half away from zero, e.g. `9°`.
#[must_use]
pub fn temp_text(c: f32, unit: TempUnit) -> String {
    let value = match unit {
        TempUnit::C => c,
        TempUnit::F => c * 9.0 / 5.0 + 32.0,
    };
    // Adding zero turns -0.0 into 0.0 so it prints as `0`.
    let rounded = value.round() + 0.0;
    format!("{rounded}°")
}

#[cfg(test)]
mod tests;
