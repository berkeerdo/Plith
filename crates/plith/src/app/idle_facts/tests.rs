use super::*;
use crate::services::Services;
use crate::services::fixture::FixtureServices;
use crate::services::fixture::idle as mock;
use plith_core::launch::Fixture;
use std::cell::Cell;
use std::rc::Rc;

fn at(hour: u16, minute: u16, second: u16) -> Stamp {
    Stamp {
        year: 2026,
        month: 11,
        day: 17,
        hour,
        minute,
        second,
        millis: 0,
    }
}

fn facts() -> Facts {
    FixtureServices::new(Fixture::Banana).facts()
}

fn silent(mut facts: Facts) -> Facts {
    facts.media.playing = false;
    facts
}

fn counting(idle: Idle) -> (Idle, Rc<Cell<u32>>) {
    let saves = Rc::new(Cell::new(0));
    let seen = Rc::clone(&saves);
    let idle = idle.with_saver(Box::new(move |_| seen.set(seen.get() + 1)));
    (idle, saves)
}

#[test]
fn the_welcome_date_and_time_follow_the_clock_and_language() {
    let idle = Idle::new(Settings::default(), Lang::Tr);
    let f = idle.welcome_facts(&facts(), at(14, 32, 10));
    assert_eq!(f.date, "SAL 17 KAS");
    assert_eq!(f.now, ((2026, 11, 17), 14 * 60 + 32));
    let english = Idle::new(Settings::default(), Lang::En);
    assert_eq!(
        english.welcome_facts(&facts(), at(14, 32, 10)).date,
        "TUE 17 NOV"
    );
}

#[test]
fn the_pill_reads_time_playing_and_seal() {
    let idle = Idle::new(Settings::default(), Lang::En);
    let f = facts();
    let pill = idle.idle_facts(&f, at(9, 5, 0));
    assert_eq!(pill.time, "09:05");
    assert_eq!(pill.seal, f.pill);
    assert_eq!(
        pill.playing,
        Some((f.media.title.clone(), f.media.artist.clone()))
    );
    assert_eq!(idle.idle_facts(&silent(f), at(9, 5, 0)).playing, None);
}

#[test]
fn weather_words_need_weather_on_and_a_forecast() {
    let now = at(14, 32, 0);
    let forecast = mock::forecast(
        (2026, 11, 17),
        WeatherStamp {
            day: (2026, 11, 17),
            hour: 14,
            minute: 0,
        },
    );
    let off = Idle::new(Settings::default(), Lang::En).with_forecast(Some(forecast.clone()));
    assert_eq!(off.idle_facts(&facts(), now).turn, None);
    assert_eq!(off.idle_facts(&facts(), now).now_sky, None);
    let settings = Settings {
        weather: true,
        ..Settings::default()
    };
    let on = Idle::new(settings, Lang::En).with_forecast(Some(forecast));
    let pill = on.idle_facts(&facts(), now);
    assert_eq!(pill.turn.as_deref(), Some("RAIN 15:00"));
    assert_eq!(pill.now_sky.as_deref(), Some("9° CLOUDY"));
}

#[test]
fn a_play_that_closes_is_saved_and_a_steady_play_is_not() {
    let (mut idle, saves) = counting(Idle::new(Settings::default(), Lang::En));
    let playing = facts();
    idle.observe(&playing, at(10, 0, 0));
    idle.observe(&playing, at(10, 5, 0));
    idle.observe(&playing, at(10, 6, 0));
    assert_eq!(saves.get(), 0, "an open play is not saved on a tick");
    idle.observe(&silent(playing), at(10, 7, 0));
    assert_eq!(saves.get(), 1, "the closed play is saved");
    idle.observe(&silent(facts()), at(10, 8, 0));
    assert_eq!(saves.get(), 1, "silence saves nothing");
}

#[test]
fn a_track_change_saves_once() {
    let (mut idle, saves) = counting(Idle::new(Settings::default(), Lang::En));
    let first = facts();
    let mut second = facts();
    second.media.title = "Another".into();
    idle.observe(&first, at(10, 0, 0));
    idle.observe(&second, at(10, 4, 0));
    assert_eq!(saves.get(), 1);
    let welcome = idle.welcome_facts(&second, at(10, 4, 30));
    assert_eq!(welcome.plays.len(), 2);
    assert!(welcome.playing);
    assert_eq!(welcome.plays[1].title, "Another");
}

#[test]
fn a_skip_under_ten_seconds_is_not_a_play() {
    let mut idle = Idle::new(Settings::default(), Lang::En);
    let mut skipped = facts();
    skipped.media.title = "Skipped".into();
    idle.observe(&skipped, at(10, 0, 0));
    idle.observe(&facts(), at(10, 0, 4));
    let welcome = idle.welcome_facts(&facts(), at(10, 0, 5));
    assert!(welcome.plays.iter().all(|p| p.title != "Skipped"));
}

#[test]
fn the_sound_head_names_the_default_output() {
    let idle = Idle::new(Settings::default(), Lang::En);
    let f = facts();
    let head = idle.welcome_facts(&f, at(12, 0, 0)).sound.unwrap();
    assert_eq!(head.output, f.pill.output);
    assert_eq!(head.value, f.pill.value);
    assert!(!head.kind.is_empty());
}

#[test]
fn a_new_forecast_is_news_once() {
    let mut idle = Idle::new(Settings::default(), Lang::En);
    let day = (2026, 11, 17);
    let stamp = WeatherStamp {
        day,
        hour: 9,
        minute: 0,
    };
    assert!(idle.set_forecast(mock::forecast(day, stamp)));
    assert!(!idle.set_forecast(mock::forecast(day, stamp)));
}
