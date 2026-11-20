//! The idle look states of the R7 mock (`docs/design/r7/karot.html`): the welcome view in its
//! variants and the six resting pill looks. The data is the banana fixture's (the mock's five
//! plays and its forecast) at a fixed day and time, so every render is the same.

use super::Content;
use plith_core::lang::{Lang, Text, date_short, device_kind, text, weekday};
use plith_core::pages::PageId;
use plith_core::pages::audio::DeviceKind;
use plith_core::pages::fixtures;
use plith_core::pages::idle_pill::{IdleFacts, idle_pill, rest_shape, sound_words, weather_words};
use plith_core::pages::welcome::{SoundHead, WelcomeFacts, welcome_page};
use plith_core::plays::Play;
use plith_core::settings::{HeadSlot, IdleLook, RunSpan, TempUnit};
use plith_core::weather::{Day, Forecast, Hour, Sky, Stamp};
use std::time::Duration;

/// The fixed day of every render.
const DAY: Day = (2026, 11, 17);
/// The day after, for the forecast's second day.
const NEXT_DAY: Day = (2026, 11, 18);
/// The fixed time of day: 14:32.
const NOW_HOUR: u8 = 14;
const NOW_MINUTE: u8 = 32;
/// The kind of the default output.
const KIND: DeviceKind = DeviceKind::Headphones;
/// A playing title too long for the Karot slot.
const LONG_TITLE: &str = "Midnight City (Eric Prydz Private Remix)";
/// Seconds into the scroll run of the mid-run render: past the first hold, travelling left.
const SCROLL_MID_S: f64 = 2.0;

fn play(start_min: u32, end_min: u32, title: &str, artist: &str) -> Play {
    Play {
        start_min,
        end_min,
        title: title.into(),
        artist: artist.into(),
    }
}

/// The mock's five plays.
fn plays() -> Vec<Play> {
    vec![
        play(546, 594, "Teardrop", "Massive Attack"),
        play(600, 642, "Holocene", "Bon Iver"),
        play(645, 684, "Retrograde", "James Blake"),
        play(792, 834, "Nightcall", "Kavinsky"),
        play(840, 872, "Avril 14th", "Aphex Twin"),
    ]
}

fn sky_at(hour: u8) -> Sky {
    match hour {
        0..=9 => Sky::Clear,
        15..=18 => Sky::Rain,
        _ => Sky::Cloud,
    }
}

/// The mock's forecast for two days, read at `read_hour:00` on the fixed day.
fn forecast(read_hour: u8) -> Forecast {
    let hours = [DAY, NEXT_DAY]
        .into_iter()
        .flat_map(|day| {
            (0..24u8).map(move |hour| Hour {
                day,
                hour,
                sky: sky_at(hour),
                temp_c: if hour == 17 { 7.0 } else { 9.0 },
                rain_pct: Some(if sky_at(hour) == Sky::Rain { 80 } else { 10 }),
            })
        })
        .collect();
    Forecast {
        hours,
        read_at: Stamp {
            day: DAY,
            hour: read_hour,
            minute: 0,
        },
    }
}

/// The welcome facts at rest: playing, weather on, forecast read at 14:00.
pub(super) fn rest(lang: Lang) -> WelcomeFacts {
    WelcomeFacts {
        now: (DAY, u32::from(NOW_HOUR) * 60 + u32::from(NOW_MINUTE)),
        date: date_short(weekday(DAY), DAY.2, DAY.1, lang),
        plays_on: true,
        plays: plays(),
        playing: true,
        forecast: Some(forecast(NOW_HOUR)),
        weather_on: true,
        span: RunSpan::Six,
        unit: TempUnit::C,
        head: HeadSlot::Auto,
        sound: Some(SoundHead {
            output: "G733".into(),
            kind: device_kind(KIND, lang).to_string(),
            value: Some(62),
            muted: false,
            flowing: true,
        }),
        paused_last: false,
        lang,
    }
}

fn welcome(facts: &WelcomeFacts, probe: Option<i32>) -> Vec<Content> {
    vec![Content::Page(PageId::Welcome, welcome_page(facts, probe))]
}

/// The pill facts at the fixed time, with the forecast turn and the playing track.
pub(super) fn idle_facts(lang: Lang) -> IdleFacts {
    let now = Stamp {
        day: DAY,
        hour: NOW_HOUR,
        minute: NOW_MINUTE,
    };
    let words = weather_words(&forecast(NOW_HOUR), now, TempUnit::C, lang);
    IdleFacts {
        time: format!("{NOW_HOUR:02}:{NOW_MINUTE:02}"),
        turn: words.turn,
        now_sky: words.now_sky,
        playing: Some(("Nightcall".into(), "Kavinsky".into())),
        seal: fixtures::pills()[0].clone(),
        sound: sound_words(
            Some(62),
            false,
            device_kind(KIND, lang),
            text(Text::Off, lang),
            lang,
        ),
        lang,
        scroll_start: None,
    }
}

/// Every welcome state and pill look in `lang`, by file name.
pub fn states(lang: Lang) -> Vec<(String, Vec<Content>)> {
    let mut stale_paused = rest(lang);
    stale_paused.forecast = Some(forecast(NOW_HOUR - 4));
    stale_paused.playing = false;
    stale_paused.paused_last = true;
    let mut weather_off = rest(lang);
    weather_off.weather_on = false;
    weather_off.forecast = None;
    let mut no_plays = rest(lang);
    no_plays.plays = Vec::new();
    no_plays.playing = false;
    let mut out = vec![
        ("welcome-rest".to_string(), welcome(&rest(lang), None)),
        ("welcome-probe-past".into(), welcome(&rest(lang), Some(10))),
        (
            "welcome-probe-future".into(),
            welcome(&rest(lang), Some(16)),
        ),
        ("welcome-stale-paused".into(), welcome(&stale_paused, None)),
        ("welcome-weather-off".into(), welcome(&weather_off, None)),
        ("welcome-no-plays".into(), welcome(&no_plays, None)),
    ];
    let facts = idle_facts(lang);
    for look in [
        IdleLook::Karot,
        IdleLook::Clock,
        IdleLook::Playing,
        IdleLook::Seal,
        IdleLook::Plain,
        IdleLook::Hidden,
    ] {
        out.push((
            format!("pill-{}", look.key()),
            vec![Content::Pill(idle_pill(look, &facts), rest_shape(look))],
        ));
    }
    // Weather off: the Karot right side falls back to the track, then to the output.
    let mut no_weather = facts;
    no_weather.turn = None;
    no_weather.now_sky = None;
    let mut silent = no_weather.clone();
    silent.playing = None;
    let mut scroll_facts = no_weather.clone();
    scroll_facts.playing = Some((LONG_TITLE.into(), "M83".into()));
    for (name, f) in [
        ("pill-karot-track", no_weather),
        ("pill-karot-sound", silent),
    ] {
        let list = idle_pill(IdleLook::Karot, &f);
        out.push((
            name.into(),
            vec![Content::Pill(list, rest_shape(IdleLook::Karot))],
        ));
    }
    out.extend(scroll_states(&scroll_facts));
    out
}

/// A long playing title as a title scroll: at rest (started at the render's wall time) and
/// mid-run (started `SCROLL_MID_S` before it, while the title travels left).
fn scroll_states(facts: &IdleFacts) -> Vec<(String, Vec<Content>)> {
    let now = fixtures::live_values().now;
    let at = |elapsed: f64| {
        let mut f = facts.clone();
        f.scroll_start = now.checked_sub(Duration::from_secs_f64(elapsed));
        vec![Content::Pill(
            idle_pill(IdleLook::Karot, &f),
            rest_shape(IdleLook::Karot),
        )]
    };
    vec![
        ("pill-karot-scroll-rest".into(), at(0.0)),
        ("pill-karot-scroll-mid".into(), at(SCROLL_MID_S)),
    ]
}
