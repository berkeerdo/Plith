//! The weather words of the resting pill: the sky now and the next turn.

use crate::lang::{Lang, sky_word};
use crate::pages::tabbar::clock_text;
use crate::settings::TempUnit;
use crate::weather::{Forecast, Stamp, is_stale, next_turn, temp_text};

/// How far ahead the next sky turn is looked for, in hours (as on the welcome view).
const TURN_WITHIN_H: u8 = 6;

/// The weather words the pill can show, both `None` without a usable forecast.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WeatherWords {
    /// The weather now, for example `9° BULUTLU`.
    pub now_sky: Option<String>,
    /// The next weather turn, for example `YAĞMUR 15:00`.
    pub turn: Option<String>,
}

/// The words for `forecast` at `now`. A forecast older than 90 minutes, or one without the
/// current hour, says nothing: the pill never guesses.
#[must_use]
pub fn weather_words(forecast: &Forecast, now: Stamp, unit: TempUnit, lang: Lang) -> WeatherWords {
    if is_stale(forecast.read_at, now) {
        return WeatherWords::default();
    }
    let Some(hour) = forecast
        .hours
        .iter()
        .find(|h| h.day == now.day && h.hour == now.hour)
    else {
        return WeatherWords::default();
    };
    WeatherWords {
        now_sky: Some(format!(
            "{} {}",
            temp_text(hour.temp_c, unit),
            sky_word(hour.sky, lang)
        )),
        turn: next_turn(forecast, (now.day, now.hour), TURN_WITHIN_H)
            .map(|(h, sky)| format!("{} {}", sky_word(sky, lang), clock_text(h, 0))),
    }
}

#[cfg(test)]
mod tests;
