//! The resting pill looks: Karot, Clock, Playing, Seal, Plain and Hidden.
//!
//! Positions come from `pill(kind)` in `docs/design/r7/karot.html` (200 x 32 DIP). Strings
//! arrive preformatted from the app, so nothing here reads the text table.

use super::audio::DeviceKind;
use super::glyphs;
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
    pub sound: Option<SoundWords>,
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

/// Gap between the speaker glyph and the text.
const GLYPH_GAP: f64 = 4.0;
/// Room the speaker glyph takes from the right-hand box.
pub(crate) const GLYPH_ROOM: f64 = glyphs::GLYPH + GLYPH_GAP;

/// The default output in short for the Karot right side, and whether it is the off word.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoundWords {
    /// `%62 · kind`, `%62`, or the off word.
    pub text: String,
    /// True when `text` is the off word: it gets no speaker glyph.
    pub muted: bool,
}

/// The default output in short for the Karot right side: `off` when muted, else `%62 · kind`,
/// or only `%62` when the kind is empty or would not leave room for the glyph. `None` without a
/// volume.
#[must_use]
pub fn sound_words(
    value: Option<u8>,
    muted: bool,
    kind: &str,
    off: &str,
    lang: Lang,
) -> Option<SoundWords> {
    if muted {
        return Some(SoundWords {
            text: off.to_string(),
            muted: true,
        });
    }
    let short = percent(value?, lang);
    let full = format!("{short} · {kind}");
    // Allowed: a few dozen characters at most.
    #[allow(clippy::cast_precision_loss)]
    let needed = full.chars().count() as f64 * VALUE_ADVANCE;
    let text = if kind.is_empty() || needed > TURN_W - GLYPH_ROOM {
        short
    } else {
        full
    };
    Some(SoundWords { text, muted: false })
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
/// else the playing title, else the default output (the only one that wears the speaker glyph,
/// unless it is the off word).
fn karot_right(f: &IdleFacts) -> Option<(&String, bool)> {
    let plain = |s| Some((s, false));
    if let Some(s) = f.turn.as_ref().or(f.now_sky.as_ref()) {
        return plain(s);
    }
    if let Some((title, _)) = &f.playing {
        return plain(title);
    }
    f.sound.as_ref().map(|s| (&s.text, !s.muted))
}

/// The speaker glyph left of the right-aligned `text`, centred on the text box.
fn speaker_before(text: &str, y: f64) -> Vec<Node> {
    // Allowed: a few dozen characters at most.
    #[allow(clippy::cast_precision_loss)]
    let width = text.chars().count() as f64 * VALUE_ADVANCE;
    let x = RIGHT_EDGE - width - GLYPH_ROOM;
    glyphs::glyph(
        DeviceKind::Speaker,
        x,
        y + TEXT_H / 2.0 - glyphs::GLYPH / 2.0,
        Ink::Muted,
    )
}

fn karot(f: &IdleFacts, y: f64) -> Vec<Node> {
    let mut nodes = vec![text(
        rect(PAD_LEFT, y, TIME_W, TEXT_H),
        &f.time,
        TextStyle::Clock,
        Ink::Ink,
        Align::Start,
    )];
    if let Some((right, glyph)) = karot_right(f) {
        nodes.push(text(
            rect(RIGHT_EDGE - TURN_W, y, TURN_W, TEXT_H),
            right,
            TextStyle::Value,
            Ink::Muted,
            Align::End,
        ));
        if glyph {
            nodes.extend(speaker_before(right, y));
        }
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
