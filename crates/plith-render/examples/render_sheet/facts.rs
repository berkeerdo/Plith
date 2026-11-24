//! Sheet facts for renders and tests: the pill previews built from the R7 mock's data (14:32 on
//! a fixed day, rain at 15:30, Nightcall playing, the first pill fixture's output at 62).

use plith_core::lang::{Lang, Text, device_kind, text};
use plith_core::pages::audio::DeviceKind;
use plith_core::pages::fixtures;
use plith_core::pages::idle_pill::{IdleFacts, idle_pill, sound_words, weather_words};
use plith_core::settings::{IdleLook, Settings, TempUnit};
use plith_core::sheet::SheetFacts;
use plith_core::weather::{Day, Forecast, Hour, Sky, Stamp};
use std::sync::Arc;

/// The fixed day of every render.
const DAY: Day = (2026, 11, 17);
/// The fixed time of day: 14:32.
const NOW: (u8, u8) = (14, 32);
/// The looks in `SheetFacts::previews` order.
const LOOKS: [IdleLook; 6] = [
    IdleLook::Karot,
    IdleLook::Clock,
    IdleLook::Playing,
    IdleLook::Seal,
    IdleLook::Plain,
    IdleLook::Hidden,
];

fn sky_at(hour: u8) -> Sky {
    match hour {
        0..=9 => Sky::Clear,
        15..=18 => Sky::Rain,
        _ => Sky::Cloud,
    }
}

/// The mock's forecast for the fixed day, read at 14:00.
fn forecast() -> Forecast {
    let hours = (0..24u8)
        .map(|hour| Hour {
            day: DAY,
            hour,
            sky: sky_at(hour),
            temp_c: if hour == 17 { 7.0 } else { 9.0 },
            rain_pct: Some(if sky_at(hour) == Sky::Rain { 80 } else { 10 }),
        })
        .collect();
    Forecast {
        hours,
        read_at: Stamp {
            day: DAY,
            hour: NOW.0,
            minute: 0,
        },
    }
}

/// The resting pill facts at the fixed time in `lang`.
pub fn idle_facts(lang: Lang) -> IdleFacts {
    let now = Stamp {
        day: DAY,
        hour: NOW.0,
        minute: NOW.1,
    };
    let words = weather_words(&forecast(), now, TempUnit::C, lang);
    IdleFacts {
        time: format!("{:02}:{:02}", NOW.0, NOW.1),
        turn: words.turn,
        now_sky: words.now_sky,
        playing: Some(("Nightcall".into(), "Kavinsky".into())),
        seal: fixtures::pills()[0].clone(),
        sound: sound_words(
            Some(62),
            false,
            device_kind(DeviceKind::Headphones, lang),
            text(Text::Off, lang),
            lang,
        ),
        lang,
        scroll_start: None,
    }
}

/// Default settings in `lang`, with the six previews built from [`idle_facts`].
pub fn sheet_facts(lang: Lang) -> SheetFacts {
    let idle = idle_facts(lang);
    SheetFacts {
        settings: Settings::default(),
        lang,
        previews: LOOKS.map(|look| Arc::new(idle_pill(look, &idle))),
        version: env!("CARGO_PKG_VERSION"),
        build_date: "2026-11-20",
        flyout_locked: false,
        settings_path: "C:\\Users\\someone\\AppData\\Local\\Plith\\settings.ini".into(),
        save_failed: false,
        autostart: false,
    }
}
