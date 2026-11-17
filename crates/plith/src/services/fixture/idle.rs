//! The idle look data of `--fixture banana`: the mock's five plays and its forecast
//! (`docs/design/r7/karot.html`), so the welcome view renders without a play history or a
//! network. The plays sit at fixed times of the day; the forecast follows the clock.

use plith_core::plays::{Day, PlayLog};
use plith_core::weather::{Forecast, Hour, Sky, Stamp};
use std::fmt::Write as _;

/// The mock's plays: start minute, end minute, title, artist.
const PLAYS: [(u32, u32, &str, &str); 5] = [
    (546, 594, "Teardrop", "Massive Attack"),
    (600, 642, "Holocene", "Bon Iver"),
    (645, 684, "Retrograde", "James Blake"),
    (792, 834, "Nightcall", "Kavinsky"),
    (840, 872, "Avril 14th", "Aphex Twin"),
];

/// The mock's plays on `day`.
#[must_use]
pub fn plays(day: Day) -> PlayLog {
    let (y, m, d) = day;
    let mut text = String::new();
    for (start, end, title, artist) in PLAYS {
        let _ = writeln!(text, "{y:04}-{m:02}-{d:02} {start} {end} {title}\t{artist}");
    }
    PlayLog::parse(&text)
}

/// The mock's sky at `hour`: clear until 10, cloud, rain from 15 to 18, cloud again.
fn sky_at(hour: u8) -> Sky {
    match hour {
        0..=9 => Sky::Clear,
        15..=18 => Sky::Rain,
        _ => Sky::Cloud,
    }
}

/// The day after `(year, month, day)`.
fn next_day((year, month, day): Day) -> Day {
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let last = match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    match (day < last, month < 12) {
        (true, _) => (year, month, day + 1),
        (false, true) => (year, month + 1, 1),
        (false, false) => (year + 1, 1, 1),
    }
}

/// The mock's forecast for `day` and the day after, read at `read_at`.
#[must_use]
pub fn forecast(day: Day, read_at: Stamp) -> Forecast {
    let hours = [day, next_day(day)]
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
    Forecast { hours, read_at }
}

#[cfg(test)]
mod tests;
