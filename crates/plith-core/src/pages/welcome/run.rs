//! The run: the time axis with the song band above it and the weather band below it.

use super::WelcomeFacts;
use super::axis::{Axis, Weather, X0, X1, ceil_i32, floor_i32};
use super::hatch::hatch;
use crate::draw::{Align, Ink, Node, StationMark, TextStyle};
use crate::lang::{Text, text as word};
use crate::pages::kit::{line, rect, station, text};
use crate::weather::{Sky, temp_text};

/// Y of the axis.
const AXIS: f64 = 150.0;
/// The song band: top and height.
const SONG_Y: f64 = 136.0;
const SONG_H: f64 = 8.0;
/// The weather band: top and height.
const BAND_Y: f64 = 156.5;
const BAND_H: f64 = 14.0;
/// How far the bit reaches above and below the axis.
const BIT_UP: f64 = 18.0;
const BIT_DOWN: f64 = 22.0;
/// The widest song label, so it stays clear of the head.
const LABEL_W: f64 = 220.0;
/// Closest two temperature labels may sit.
const TEMP_GAP: f64 = 30.0;

/// The run nodes. `hide_label` drops the song label (the head shows the song instead).
pub(super) fn run(
    f: &WelcomeFacts,
    weather: Option<&Weather>,
    probe: Option<i32>,
    hide_label: bool,
) -> Vec<Node> {
    let ax = Axis::new(f);
    let mut out = Vec::new();
    song_band(&mut out, f, &ax, hide_label);
    let nx = ax.x(ax.now);
    out.push(line(vec![(X0, AXIS), (nx, AXIS)], 1.0, Ink::Track));
    out.push(dashed(vec![(nx, AXIS), (X1, AXIS)], Ink::Track));
    if let Some(w) = weather {
        weather_band(&mut out, f, &ax, w);
    }
    depth(&mut out, &ax);
    out.push(line(
        vec![(nx, AXIS - BIT_UP), (nx, AXIS + BIT_DOWN)],
        1.0,
        Ink::Ink,
    ));
    out.push(station(nx, AXIS, StationMark::Filled, Ink::Ink));
    if let Some(hour) = probe {
        let x = ax.x(f64::from(hour) * 60.0 + 30.0);
        if (X0..=X1).contains(&x) {
            out.push(dashed(
                vec![(x, AXIS - BIT_UP), (x, AXIS + BIT_DOWN)],
                Ink::Muted,
            ));
        }
    }
    out
}

fn dashed(points: Vec<(f64, f64)>, ink: Ink) -> Node {
    Node::Stroke {
        points,
        width: 1.0,
        ink,
        dashed: true,
    }
}

fn song_band(out: &mut Vec<Node>, f: &WelcomeFacts, ax: &Axis, hide_label: bool) {
    let Some(last) = f.plays.len().checked_sub(1) else {
        out.push(text(
            rect(X0, AXIS - 24.0, LABEL_W, 12.0),
            word(Text::NothingPlayed, f.lang),
            TextStyle::Label,
            Ink::Faint,
            Align::Start,
        ));
        return;
    };
    let mut last_shown = false;
    for (i, p) in f.plays.iter().enumerate() {
        let xa = ax.x(f64::from(p.start_min)).max(X0);
        let xb = ax.x(f64::from(p.end_min).min(ax.now)).min(X1);
        if xb <= xa {
            continue;
        }
        last_shown = i == last;
        let ink = if f.playing && i == last {
            Ink::Flow
        } else {
            Ink::Muted
        };
        out.push(Node::Fill {
            rect: rect(xa, SONG_Y, (xb - xa - 1.0).max(1.0), SONG_H),
            radius: 0.0,
            ink,
        });
    }
    if last_shown && !hide_label {
        song_label(out, f, ax);
    }
}

/// The title and artist of the last play, right-aligned at the start of its interval.
fn song_label(out: &mut Vec<Node>, f: &WelcomeFacts, ax: &Axis) {
    let Some(p) = f.plays.last() else { return };
    let right = (ax.x(f64::from(p.start_min)) - 2.0).clamp(X0 + 60.0, X1);
    let left = (right - LABEL_W).max(X0);
    let artist = if f.playing {
        p.artist.clone()
    } else {
        format!("{} · {}", p.artist, word(Text::Paused, f.lang))
    };
    out.push(text(
        rect(left, AXIS - 34.0, right - left, 14.0),
        &p.title,
        TextStyle::Body,
        Ink::Ink,
        Align::End,
    ));
    out.push(text(
        rect(left, AXIS - 46.0, right - left, 12.0),
        &artist,
        TextStyle::Value,
        Ink::Faint,
        Align::End,
    ));
}

/// A run of equal sky, in hours of the run (`to` is exclusive).
struct Section {
    from: i32,
    to: i32,
    sky: Sky,
    temp_c: f32,
}

fn sections(w: &Weather, ax: &Axis) -> Vec<Section> {
    let mut out: Vec<Section> = Vec::new();
    for h in ax.cells() {
        let Some(hour) = w.at(h) else { continue };
        match out.last_mut() {
            Some(s) if s.to == h && s.sky == hour.sky => s.to = h + 1,
            _ => out.push(Section {
                from: h,
                to: h + 1,
                sky: hour.sky,
                temp_c: hour.temp_c,
            }),
        }
    }
    out
}

fn weather_band(out: &mut Vec<Node>, f: &WelcomeFacts, ax: &Axis, w: &Weather) {
    let nx = ax.x(ax.now);
    let all = sections(w, ax);
    for s in &all {
        let xa = ax.x(f64::from(s.from) * 60.0).max(X0);
        let xb = ax.x(f64::from(s.to) * 60.0).min(X1);
        if xb <= xa {
            continue;
        }
        let cut = nx.clamp(xa, xb);
        block(out, xa, cut, s.sky, true);
        block(out, cut, xb, s.sky, false);
    }
    let Some(now) = w.at(w.now_hour) else { return };
    let mut shown = nx - TEMP_GAP;
    let turns = all
        .iter()
        .filter(|s| f64::from(s.from) * 60.0 > ax.now)
        .map(|s| (ax.x(f64::from(s.from) * 60.0), s.temp_c));
    for (i, (x, c)) in std::iter::once((nx, now.temp_c)).chain(turns).enumerate() {
        if x >= X1 - 10.0 || x < shown + TEMP_GAP {
            continue;
        }
        shown = x;
        out.push(text(
            rect(x + 3.0, AXIS + 24.0, 40.0, 12.0),
            &temp_text(c, f.unit),
            TextStyle::Value,
            if i == 0 { Ink::Ink } else { Ink::Faint },
            Align::Start,
        ));
    }
}

/// One weather section between `xa` and `xb`: logged ground is filled and solid, the forecast
/// is open and dashed. Marks sit on a grid anchored at the page origin, like a tiled pattern.
fn block(out: &mut Vec<Node>, xa: f64, xb: f64, sky: Sky, logged: bool) {
    let w = xb - xa - 1.0;
    if w <= 0.0 {
        return;
    }
    let (x, y) = (xa + 0.5, BAND_Y);
    let r = rect(x, y, w, BAND_H);
    if logged {
        out.push(Node::Fill {
            rect: r,
            radius: 0.0,
            ink: Ink::FloorHigh,
        });
    }
    hatch(
        out,
        sky,
        (x + 1.0, y + 1.0, x + w - 1.0, y + BAND_H - 1.0),
        logged,
    );
    out.push(Node::Stroke {
        points: vec![
            (x, y),
            (x + w, y),
            (x + w, y + BAND_H),
            (x, y + BAND_H),
            (x, y),
        ],
        width: 1.0,
        ink: Ink::Track,
        dashed: !logged,
    });
}

/// Depth ticks every hour, labels every second hour.
fn depth(out: &mut Vec<Node>, ax: &Axis) {
    let first = ceil_i32(ax.lo / 60.0);
    let last = floor_i32((ax.lo + ax.span) / 60.0);
    for h in first..=last {
        let x = ax.x(f64::from(h) * 60.0);
        out.push(line(
            vec![(x, AXIS - 3.0), (x, AXIS + 3.0)],
            1.0,
            Ink::Track,
        ));
        if h % 2 == 0 {
            let label = u8::try_from(h.rem_euclid(24)).unwrap_or(0);
            out.push(text(
                rect(x - 12.0, AXIS + 42.0, 24.0, 12.0),
                &format!("{label:02}"),
                TextStyle::Label,
                Ink::Faint,
                Align::Center,
            ));
        }
    }
}
