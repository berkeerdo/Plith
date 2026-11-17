//! The time axis of the run and the forecast as the run sees it.

use super::WelcomeFacts;
use crate::settings::RunSpan;
use crate::weather::{Forecast, Hour};

/// Left and right end of the run.
pub(super) const X0: f64 = 30.0;
pub(super) const X1: f64 = 590.0;
/// The time axis: which minutes the run shows and where they land.
#[derive(Debug, Clone, Copy)]
pub(super) struct Axis {
    pub(super) lo: f64,
    pub(super) span: f64,
    pub(super) now: f64,
}

impl Axis {
    pub(super) fn new(f: &WelcomeFacts) -> Self {
        let now = f64::from(f.now.1);
        let (lo, span) = match f.span {
            RunSpan::Three => (now - 180.0, 360.0),
            RunSpan::Six => (now - 360.0, 720.0),
            RunSpan::Day => (0.0, 1440.0),
        };
        Self { lo, span, now }
    }

    /// The x of a minute of the day (unwrapped: may be below 0 or past 1440).
    pub(super) fn x(&self, minute: f64) -> f64 {
        X0 + (minute - self.lo) / self.span * (X1 - X0)
    }

    /// The minute under an x.
    pub(super) fn minute_at(&self, x: f64) -> f64 {
        self.lo + (x - X0) / (X1 - X0) * self.span
    }

    /// The hours whose cells touch the run, as `first..end`.
    pub(super) fn cells(&self) -> std::ops::Range<i32> {
        floor_i32(self.lo / 60.0)..ceil_i32((self.lo + self.span) / 60.0)
    }
}

#[allow(clippy::cast_possible_truncation)]
pub(super) fn floor_i32(v: f64) -> i32 {
    v.floor() as i32
}

pub(super) fn ceil_i32(v: f64) -> i32 {
    -floor_i32(-v)
}

/// A usable forecast: weather is on, there is a forecast and it holds the current hour.
pub(super) struct Weather<'a> {
    pub(super) forecast: &'a Forecast,
    idx: usize,
    pub(super) now_hour: i32,
}

impl<'a> Weather<'a> {
    pub(super) fn of(f: &'a WelcomeFacts) -> Option<Self> {
        let forecast = f.forecast.as_ref().filter(|_| f.weather_on)?;
        let (day, minute) = f.now;
        let hour = u8::try_from(minute / 60).ok()?;
        let idx = forecast
            .hours
            .iter()
            .position(|h| h.day == day && h.hour == hour)?;
        Some(Self {
            forecast,
            idx,
            now_hour: i32::from(hour),
        })
    }

    /// The forecast hour at an hour of the run (unwrapped, 0 is today's midnight).
    pub(super) fn at(&self, hour: i32) -> Option<&'a Hour> {
        let i = i64::try_from(self.idx).ok()? + i64::from(hour - self.now_hour);
        self.forecast.hours.get(usize::try_from(i).ok()?)
    }
}
