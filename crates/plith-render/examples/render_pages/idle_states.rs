//! The idle look states of the R7 mock (`docs/design/r7/karot.html`): the welcome view in its
//! variants and the six resting pill looks. The data is the banana fixture's (the mock's five
//! plays and its forecast) at a fixed day and time, so every render is the same.

use super::Content;
use plith_core::lang::Lang;
use plith_core::pages::PageId;
use plith_core::pages::fixtures;
use plith_core::pages::idle_pill::{IdleFacts, idle_pill, rest_shape, weather_words};
use plith_core::pages::welcome::{SoundHead, WelcomeFacts, welcome_page};
use plith_core::plays::Play;
use plith_core::settings::{HeadSlot, IdleLook, RunSpan, TempUnit};
use plith_core::weather::{Day, Forecast, Hour, Sky, Stamp};

/// The fixed day of every render.
const DAY: Day = (2026, 11, 17);
/// The day after, for the forecast's second day.
const NEXT_DAY: Day = (2026, 11, 18);
/// The fixed time of day: 14:32.
const NOW_HOUR: u8 = 14;
const NOW_MINUTE: u8 = 32;

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
fn rest() -> WelcomeFacts {
    WelcomeFacts {
        now: (DAY, u32::from(NOW_HOUR) * 60 + u32::from(NOW_MINUTE)),
        date: "SAL 17 KAS".into(),
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
            kind: "KULAKLIK".into(),
            value: Some(62),
            muted: false,
            flowing: true,
        }),
        paused_last: false,
        lang: Lang::Tr,
    }
}

fn welcome(facts: &WelcomeFacts, probe: Option<i32>) -> Vec<Content> {
    vec![Content::Page(PageId::Welcome, welcome_page(facts, probe))]
}

/// The pill facts at the fixed time, with the forecast turn and the playing track.
fn idle_facts() -> IdleFacts {
    let now = Stamp {
        day: DAY,
        hour: NOW_HOUR,
        minute: NOW_MINUTE,
    };
    let words = weather_words(&forecast(NOW_HOUR), now, TempUnit::C, Lang::Tr);
    IdleFacts {
        time: format!("{NOW_HOUR:02}:{NOW_MINUTE:02}"),
        turn: words.turn,
        now_sky: words.now_sky,
        playing: Some(("Nightcall".into(), "Kavinsky".into())),
        seal: fixtures::pills()[0].clone(),
        lang: Lang::Tr,
    }
}

/// Every welcome state and pill look, by file name.
pub fn states() -> Vec<(String, Vec<Content>)> {
    let mut stale_paused = rest();
    stale_paused.forecast = Some(forecast(NOW_HOUR - 4));
    stale_paused.playing = false;
    stale_paused.paused_last = true;
    let mut weather_off = rest();
    weather_off.weather_on = false;
    weather_off.forecast = None;
    let mut no_plays = rest();
    no_plays.plays = Vec::new();
    no_plays.playing = false;
    let mut out = vec![
        ("welcome-rest".to_string(), welcome(&rest(), None)),
        ("welcome-probe-past".into(), welcome(&rest(), Some(10))),
        ("welcome-probe-future".into(), welcome(&rest(), Some(16))),
        ("welcome-stale-paused".into(), welcome(&stale_paused, None)),
        ("welcome-weather-off".into(), welcome(&weather_off, None)),
        ("welcome-no-plays".into(), welcome(&no_plays, None)),
    ];
    let facts = idle_facts();
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
    out
}
