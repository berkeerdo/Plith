//! The resting pill looks: Karot, Clock, Playing, Seal, Plain and Hidden.
//!
//! Positions come from `pill(kind)` in `docs/design/r7/karot.html` (200 x 32 DIP). Strings
//! arrive preformatted from the app, so nothing here reads the text table.

use super::kit::{rect, station, text};
use super::pill::{PillFacts, pill};
use crate::draw::{Align, DisplayList, Ink, Node, StationMark, TextStyle};
use crate::geometry::IslandShape;
use crate::lang::{Lang, percent};
use crate::settings::IdleLook;
use crate::shapes;

mod weather;
pub use weather::{WeatherWords, weather_words};

/// What the resting looks can show.
#[derive(Debug, Clone, PartialEq)]
pub struct IdleFacts {
    /// The time of day, for example `14:05`.
    pub time: String,
    /// The next weather turn, for example `YAĞMUR 15:30`.
    pub turn: Option<String>,
    /// The weather now, for example `9° BULUTLU`.
    pub now_sky: Option<String>,
    /// The playing track as (title, artist).
    pub playing: Option<(String, String)>,
    /// The facts of the R3 seal.
    pub seal: PillFacts,
    /// The default output in short, for example `%62 · KULAKLIK` or `KAPALI`.
    pub sound: Option<String>,
    /// The interface language.
    pub lang: Lang,
}

/// Left padding of the time.
const PAD_LEFT: f64 = 16.0;
/// Right edge of the right-hand text.
const RIGHT_EDGE: f64 = 184.0;
/// Station x and the text start of the Playing look.
const STATION_X: f64 = 18.0;
const TRACK_X: f64 = 30.0;
/// Text box height.
const TEXT_H: f64 = 14.0;
/// Width of the time box in Karot.
const TIME_W: f64 = 56.0;
/// Width of the right-hand text in Karot.
pub(crate) const TURN_W: f64 = 112.0;
/// Martian Mono at 10 DIP with the tracking: a conservative advance per character.
const VALUE_ADVANCE: f64 = 7.6;

/// The default output in short for the Karot right side: `off` when muted, else `%62 · kind`,
/// or only `%62` when the kind is empty or would not fit the box. `None` without a volume.
#[must_use]
pub fn sound_words(
    value: Option<u8>,
    muted: bool,
    kind: &str,
    off: &str,
    lang: Lang,
) -> Option<String> {
    if muted {
        return Some(off.to_string());
    }
    let short = percent(value?, lang);
    if kind.is_empty() {
        return Some(short);
    }
    let full = format!("{short} · {kind}");
    // Allowed: a few dozen characters at most.
    #[allow(clippy::cast_precision_loss)]
    let needed = full.chars().count() as f64 * VALUE_ADVANCE;
    Some(if needed <= TURN_W { full } else { short })
}

/// The display list of the resting pill for `look`.
#[must_use]
pub fn idle_pill(look: IdleLook, f: &IdleFacts) -> DisplayList {
    let (w, h) = (shapes::REST.width, shapes::REST.height);
    let y = h / 2.0 - TEXT_H / 2.0;
    let nodes = match look {
        IdleLook::Karot => karot(f, y),
        IdleLook::Clock => vec![clock(f, y, w)],
        IdleLook::Playing => playing(f, y, w),
        IdleLook::Seal => return pill(&f.seal, f.lang),
        IdleLook::Plain => Vec::new(),
        IdleLook::Hidden => {
            return DisplayList {
                nodes: Vec::new(),
                size: (shapes::REST_HIDDEN.width, shapes::REST_HIDDEN.height),
            };
        }
    };
    DisplayList {
        nodes,
        size: (w, h),
    }
}

/// The island shape the app rests in for `look`.
#[must_use]
pub fn rest_shape(look: IdleLook) -> IslandShape {
    if look == IdleLook::Hidden {
        shapes::REST_HIDDEN
    } else {
        shapes::REST
    }
}

/// The Karot right-hand text, like the welcome head's `Auto` slot: the weather when it is on,
/// else the playing title, else the default output.
fn karot_right(f: &IdleFacts) -> Option<&String> {
    f.turn
        .as_ref()
        .or(f.now_sky.as_ref())
        .or(f.playing.as_ref().map(|(title, _)| title))
        .or(f.sound.as_ref())
}

fn karot(f: &IdleFacts, y: f64) -> Vec<Node> {
    let mut nodes = vec![text(
        rect(PAD_LEFT, y, TIME_W, TEXT_H),
        &f.time,
        TextStyle::Clock,
        Ink::Ink,
        Align::Start,
    )];
    if let Some(right) = karot_right(f) {
        nodes.push(text(
            rect(RIGHT_EDGE - TURN_W, y, TURN_W, TEXT_H),
            right,
            TextStyle::Value,
            Ink::Muted,
            Align::End,
        ));
    }
    nodes
}

fn clock(f: &IdleFacts, y: f64, w: f64) -> Node {
    text(
        rect(0.0, y, w, TEXT_H),
        &f.time,
        TextStyle::Clock,
        Ink::Ink,
        Align::Center,
    )
}

fn playing(f: &IdleFacts, y: f64, w: f64) -> Vec<Node> {
    let Some((title, artist)) = &f.playing else {
        return vec![clock(f, y, w)];
    };
    let line = if artist.is_empty() {
        title.clone()
    } else {
        format!("{title} \u{b7} {artist}")
    };
    vec![
        station(
            STATION_X,
            shapes::REST.height / 2.0,
            StationMark::Filled,
            Ink::Flow,
        ),
        text(
            rect(TRACK_X, y, RIGHT_EDGE - TRACK_X, TEXT_H),
            &line,
            TextStyle::Body,
            Ink::Muted,
            Align::Start,
        ),
    ]
}

#[cfg(test)]
mod tests;
