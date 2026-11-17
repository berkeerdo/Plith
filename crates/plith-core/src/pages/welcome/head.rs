//! The head: the time and date at the left, three lines at the right that read the pointer's
//! hour or the chosen slot.

use std::fmt::Write as _;

use super::WelcomeFacts;
use super::axis::Weather;
use crate::draw::{Align, Ink, Node, StationMark, TextStyle};
use crate::lang::{Text, sky_word, songs, text as word};
use crate::pages::kit::{rect, station, text};
use crate::pages::tabbar::clock_text;
use crate::plays::Play;
use crate::settings::HeadSlot;
use crate::weather::{Stamp, is_stale, next_turn, temp_text};

/// Right edge of the right-hand lines.
const RIGHT: f64 = 590.0;
/// Width of a right-hand line.
const LINE_W: f64 = 240.0;
/// Where the sound station sits and how far the output name stays clear of it.
const STATION_X: f64 = 586.5;
const STATION_Y: f64 = 74.0;
const NAME_RIGHT: f64 = 578.0;
/// Top and height of the three right-hand lines (baselines 78, 96 and 112).
const SLOTS: [(f64, f64); 3] = [(65.0, 16.0), (86.0, 12.0), (102.0, 12.0)];
/// How far ahead the next sky turn is looked for, in hours.
const TURN_WITHIN_H: u8 = 6;

/// One right-hand line.
struct Line {
    s: String,
    style: TextStyle,
    ink: Ink,
    right: f64,
}

type Lines = [Option<Line>; 3];

#[allow(
    clippy::unnecessary_wraps,
    reason = "the line slots of a head are options, so every builder returns one"
)]
fn line(s: String, style: TextStyle, ink: Ink) -> Option<Line> {
    Some(Line {
        s,
        style,
        ink,
        right: RIGHT,
    })
}

/// The slot the head shows at rest: `Auto` is weather when weather is usable, else sound.
pub(super) fn slot(f: &WelcomeFacts, weather: Option<&Weather>) -> HeadSlot {
    match f.head {
        HeadSlot::Auto if weather.is_some() => HeadSlot::Weather,
        HeadSlot::Auto => HeadSlot::Sound,
        other => other,
    }
}

/// The head nodes.
pub(super) fn head(f: &WelcomeFacts, weather: Option<&Weather>, probe: Option<i32>) -> Vec<Node> {
    let (hour, minute) = (f.now.1 / 60, f.now.1 % 60);
    let clock = clock_text(to_u8(hour), to_u8(minute));
    let mut out = vec![
        text(
            rect(30.0, 62.0, 150.0, 34.0),
            &clock,
            TextStyle::Hero,
            Ink::Ink,
            Align::Start,
        ),
        text(
            rect(32.0, 100.0, 140.0, 12.0),
            &f.date,
            TextStyle::Label,
            Ink::Faint,
            Align::Start,
        ),
    ];
    let lines = match (probe, slot(f, weather)) {
        (Some(h), _) => probe_lines(f, weather, h),
        (None, HeadSlot::Weather) => weather.map(|w| weather_lines(f, w)).unwrap_or_default(),
        (None, HeadSlot::Sound) => sound_lines(f, &mut out),
        (None, HeadSlot::Playing) => playing_lines(f),
        (None, _) => Lines::default(),
    };
    for (i, l) in lines.into_iter().enumerate() {
        let Some(l) = l else { continue };
        let (top, h) = SLOTS[i];
        out.push(text(
            rect(l.right - LINE_W, top, LINE_W, h),
            &l.s,
            l.style,
            l.ink,
            Align::End,
        ));
    }
    out
}

fn to_u8(v: u32) -> u8 {
    u8::try_from(v).unwrap_or(0)
}

fn hh(hour: i32) -> String {
    format!("{:02}:00", hour.rem_euclid(24))
}

fn stamp(f: &WelcomeFacts) -> Stamp {
    Stamp {
        day: f.now.0,
        hour: to_u8(f.now.1 / 60),
        minute: to_u8(f.now.1 % 60),
    }
}

fn read_line(f: &WelcomeFacts, w: &Weather) -> Option<Line> {
    let at = w.forecast.read_at;
    let mut s = format!(
        "{} {}",
        word(Text::Read, f.lang),
        clock_text(at.hour, at.minute)
    );
    if is_stale(at, stamp(f)) {
        s.push_str(" · ");
        s.push_str(word(Text::Old, f.lang));
    }
    line(s, TextStyle::Label, Ink::Faint)
}

/// The weather now, the next turn and the reading time.
fn weather_lines(f: &WelcomeFacts, w: &Weather) -> Lines {
    let Some(now) = w.at(w.now_hour) else {
        return Lines::default();
    };
    let stale = is_stale(w.forecast.read_at, stamp(f));
    let first = format!(
        "{} · {}",
        temp_text(now.temp_c, f.unit),
        sky_word(now.sky, f.lang)
    );
    let turn = next_turn(w.forecast, (f.now.0, stamp(f).hour), TURN_WITHIN_H).map(|(h, sky)| {
        let abs = w.now_hour + (i32::from(h) - w.now_hour).rem_euclid(24);
        let temp = w.at(abs).map_or(String::new(), |t| {
            format!(" · {}", temp_text(t.temp_c, f.unit))
        });
        format!("{} {}{temp}", hh(abs), sky_word(sky, f.lang))
    });
    [
        line(
            first,
            TextStyle::Name,
            if stale { Ink::Muted } else { Ink::Ink },
        ),
        turn.and_then(|s| line(s, TextStyle::Label, Ink::Muted)),
        read_line(f, w),
    ]
}

fn sound_lines(f: &WelcomeFacts, out: &mut Vec<Node>) -> Lines {
    let Some(s) = &f.sound else {
        return Lines::default();
    };
    let mark = if s.flowing { Ink::Flow } else { Ink::Ink };
    out.push(station(STATION_X, STATION_Y, StationMark::Filled, mark));
    let second = if s.muted {
        word(Text::Off, f.lang).to_string()
    } else {
        match s.value {
            Some(v) => format!("%{v} · {}", s.kind),
            None => s.kind.clone(),
        }
    };
    [
        Some(Line {
            s: s.output.clone(),
            style: TextStyle::Name,
            ink: Ink::Ink,
            right: NAME_RIGHT,
        }),
        line(second, TextStyle::Label, Ink::Muted),
        line(
            word(Text::DefaultOutput, f.lang).to_string(),
            TextStyle::Label,
            Ink::Faint,
        ),
    ]
}

/// The playing track: title and artist. No elapsed time: the facts are rebuilt only on an event,
/// so a drawn time would freeze between them, and the head has no live text node to put it in.
fn playing_lines(f: &WelcomeFacts) -> Lines {
    let Some(p) = f.plays.last().filter(|_| f.playing) else {
        return Lines::default();
    };
    [
        line(p.title.clone(), TextStyle::Name, Ink::Ink),
        line(p.artist.clone(), TextStyle::Value, Ink::Muted),
        None,
    ]
}

/// What the hour under the pointer says: plays for a past hour, the forecast for a later one.
fn probe_lines(f: &WelcomeFacts, weather: Option<&Weather>, hour: i32) -> Lines {
    if (hour + 1) * 60 <= i32::try_from(f.now.1).unwrap_or(0) {
        past_lines(f, hour)
    } else {
        weather
            .map(|w| future_lines(f, w, hour))
            .unwrap_or_default()
    }
}

fn overlap(p: &Play, from: i32, to: i32) -> i32 {
    let a = i32::try_from(p.start_min).unwrap_or(i32::MAX).max(from);
    let b = i32::try_from(p.end_min).unwrap_or(i32::MAX).min(to);
    b - a
}

fn past_lines(f: &WelcomeFacts, hour: i32) -> Lines {
    if !f.plays_on {
        return Lines::default();
    }
    let (from, to) = (hour * 60, hour * 60 + 60);
    let inside: Vec<(i32, &Play)> = f
        .plays
        .iter()
        .map(|p| (overlap(p, from, to), p))
        .filter(|(o, _)| *o > 0)
        .collect();
    // On a tie the earlier play wins.
    let main = inside.iter().rev().max_by_key(|(o, _)| *o).map(|(_, p)| *p);
    [
        main.and_then(|p| {
            line(
                format!("{} · {}", p.title, p.artist),
                TextStyle::Name,
                Ink::Ink,
            )
        }),
        line(
            format!(
                "{} – {} · {}",
                hh(hour),
                hh(hour + 1),
                songs(inside.len(), f.lang)
            ),
            TextStyle::Label,
            Ink::Muted,
        ),
        line(
            word(Text::Earlier, f.lang).to_string(),
            TextStyle::Label,
            Ink::Faint,
        ),
    ]
}

fn future_lines(f: &WelcomeFacts, w: &Weather, hour: i32) -> Lines {
    let Some(h) = w.at(hour) else {
        return Lines::default();
    };
    let mut first = format!(
        "{} · {}",
        sky_word(h.sky, f.lang),
        temp_text(h.temp_c, f.unit)
    );
    if let Some(pct) = h.rain_pct {
        let _ = write!(first, " · %{pct}");
    }
    [
        line(first, TextStyle::Name, Ink::Ink),
        line(
            format!("{} · {}", hh(hour), word(Text::Forecast, f.lang)),
            TextStyle::Label,
            Ink::Muted,
        ),
        read_line(f, w),
    ]
}
