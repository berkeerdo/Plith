use super::*;
use crate::weather::{Hour, Sky};

const DAY: (u16, u16, u16) = (2026, 11, 17);

fn forecast(read_hour: u8) -> Forecast {
    let sky_at = |hour: u8| match hour {
        15..=18 => Sky::Rain,
        _ => Sky::Cloud,
    };
    Forecast {
        hours: (0..24u8)
            .map(|hour| Hour {
                day: DAY,
                hour,
                sky: sky_at(hour),
                temp_c: 9.0,
                rain_pct: None,
            })
            .collect(),
        read_at: Stamp {
            day: DAY,
            hour: read_hour,
            minute: 0,
        },
    }
}

fn at(hour: u8) -> Stamp {
    Stamp {
        day: DAY,
        hour,
        minute: 5,
    }
}

#[test]
fn the_turn_and_the_sky_now_read_in_turkish() {
    let words = weather_words(&forecast(14), at(14), TempUnit::C, Lang::Tr);
    assert_eq!(words.now_sky.as_deref(), Some("9° BULUTLU"));
    assert_eq!(words.turn.as_deref(), Some("YAĞMUR 15:00"));
}

#[test]
fn english_and_fahrenheit() {
    let words = weather_words(&forecast(14), at(14), TempUnit::F, Lang::En);
    assert_eq!(words.now_sky.as_deref(), Some("48° CLOUDY"));
    assert_eq!(words.turn.as_deref(), Some("RAIN 15:00"));
}

#[test]
fn no_turn_within_six_hours_leaves_the_sky_now() {
    let words = weather_words(&forecast(2), at(2), TempUnit::C, Lang::En);
    assert!(words.now_sky.is_some());
    assert_eq!(words.turn, None);
}

#[test]
fn a_stale_forecast_says_nothing() {
    let words = weather_words(&forecast(8), at(14), TempUnit::C, Lang::En);
    assert_eq!(words, WeatherWords::default());
}

#[test]
fn a_forecast_without_the_current_hour_says_nothing() {
    let mut other_day = at(14);
    other_day.day = (2026, 11, 19);
    let words = weather_words(&forecast(14), other_day, TempUnit::C, Lang::En);
    assert_eq!(words, WeatherWords::default());
}
