//! Where every own string is drawn: its text style and the room its place leaves.
//!
//! The renderer lays each string out with its own bundled fonts and compares the width with the
//! budget (`plith-render/tests/fit.rs`), so a translation that does not fit fails a test instead
//! of being cut with an ellipsis on screen. A budget is the box width the layout states, else
//! the room the layout leaves, else the width of the Turkish string the R4 renders proved.

use super::{
    Lang, Text, date_short, device_kind, files, percent, setup, sheet, sky_word, songs, text,
};
use crate::draw::TextStyle;
use crate::pages::audio::{
    CUT_LABEL_W, DeviceKind, OUTPUT_NAME_W, SOURCE_NAME_W, VALUE_CHAR_W, VALUE_SLACK,
};
use crate::pages::idle_pill::{GLYPH_ROOM, TURN_W, sound_words};
use crate::pages::system::{CUT_FROM, CUT_TO, LABEL_W, RIGHT, VALUE_W as VALUE_BOX_W, X0};
use crate::pages::tabbar::LABEL_W as TAB_W;
use crate::pages::welcome::{LINE_W, RUN_LABEL_W};
use crate::pages::{MARGIN_LEFT, PAGE_W, pill};
use crate::shelf::layout::{CLEAR_W, FOOTER_X, SLOT_MIN_W, TAG_W, UNDO_W, UNDO_X};
use crate::weather::Sky;

/// The OSD value slot (`plith-osd`, `VALUE_LEFT` 236 to the 284 edge). A test in `plith-osd`
/// (`paint/tests.rs`) asserts its slot equals this.
pub const OSD_VALUE_W: f64 = 48.0;
/// The welcome date line box.
const DATE_W: f64 = 140.0;

/// How much room a place leaves.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Budget {
    /// A width in DIP.
    Dip(f64),
    /// No stated box: the string must be no wider than the Turkish one of the same place.
    LikeTurkish,
}

/// One drawn string, the style it is set in and the room it has.
#[derive(Debug, Clone, PartialEq)]
pub struct Place {
    /// A readable name of the place, stable across languages (the Turkish twin is found by it).
    pub name: String,
    /// The string as drawn, before upper-casing.
    pub text: String,
    /// The text style of the place.
    pub style: TextStyle,
    /// The room it has.
    pub budget: Budget,
}

fn place(name: &str, text: impl Into<String>, style: TextStyle, budget: Budget) -> Place {
    Place {
        name: name.to_owned(),
        text: text.into(),
        style,
        budget,
    }
}

const KINDS: [DeviceKind; 3] = [
    DeviceKind::Headphones,
    DeviceKind::Speaker,
    DeviceKind::Display,
];

/// Every place that draws own copy, with the strings in `lang`.
#[must_use]
pub fn places(lang: Lang) -> Vec<Place> {
    let mut out = Vec::new();
    let t = |id| text(id, lang);
    let label = TextStyle::Label;
    let page_box = Budget::Dip(PAGE_W - 2.0 * MARGIN_LEFT);
    for (name, id) in [
        ("tab audio", Text::TabAudio),
        ("tab media", Text::TabMedia),
        ("tab shelf", Text::TabShelf),
        ("tab system", Text::TabSystem),
    ] {
        out.push(place(name, t(id), label, Budget::Dip(TAB_W)));
    }
    for (name, id) in [
        ("footer normal", Text::FooterNormal),
        ("footer picking", Text::FooterPicking),
        ("footer outputs", Text::FooterOutputs),
        ("other outputs", Text::OtherOutputs),
        ("shelf hint", Text::ShelfHint),
    ] {
        out.push(place(name, t(id), label, page_box));
    }
    out.push(place(
        "ddc note",
        t(Text::DdcOff),
        label,
        Budget::Dip(RIGHT - X0),
    ));
    out.push(place(
        "shelf drop",
        t(Text::ShelfDrop),
        TextStyle::Name,
        page_box,
    ));
    out.push(place(
        "no output",
        t(Text::NoOutput),
        TextStyle::Name,
        page_box,
    ));
    out.extend(output_places(lang));
    out.extend(muted_places(lang));
    out.extend(page_places(lang));
    out.extend(pill_places(lang));
    out.extend(welcome_places(lang));
    out.extend(shelf_places(lang));
    out.extend(sheet::fit::places(lang));
    out.extend(setup::fit::places(lang));
    out
}

/// The shelf's own copy: the slot word, the undo word and the footer line.
fn shelf_places(lang: Lang) -> Vec<Place> {
    let label = TextStyle::Label;
    let t = |id| text(id, lang);
    vec![
        place(
            "shelf slot",
            t(Text::ShelfSlot),
            label,
            Budget::Dip(SLOT_MIN_W),
        ),
        place("shelf undo", t(Text::ShelfUndo), label, Budget::Dip(UNDO_W)),
        place(
            "shelf clear",
            t(Text::ShelfClear),
            label,
            Budget::Dip(CLEAR_W),
        ),
        place(
            "shelf folder tag",
            t(Text::ShelfFolder),
            label,
            Budget::Dip(TAG_W),
        ),
        place(
            "shelf file tag",
            t(Text::ShelfFile),
            label,
            Budget::Dip(TAG_W),
        ),
        place(
            "shelf footer",
            format!("{} \u{b7} {}", files(8, lang), t(Text::ShelfFooter)),
            label,
            Budget::Dip(UNDO_X - FOOTER_X - 8.0),
        ),
    ]
}

/// The media page copy, the system row labels and the audio source names.
fn page_places(lang: Lang) -> Vec<Place> {
    let mut out = Vec::new();
    let t = |id| text(id, lang);
    let page_box = Budget::Dip(PAGE_W - 2.0 * MARGIN_LEFT);
    out.push(place(
        "media empty title",
        t(Text::NothingPlaying),
        TextStyle::Strong,
        page_box,
    ));
    out.push(place(
        "media hint",
        t(Text::MediaHint),
        TextStyle::Body,
        page_box,
    ));
    let name_box = Budget::Dip(LABEL_W);
    out.push(place(
        "brightness label",
        t(Text::Brightness),
        TextStyle::Name,
        name_box,
    ));
    out.push(place(
        "microphone label",
        t(Text::Microphone),
        TextStyle::Name,
        name_box,
    ));
    out.push(place(
        "source name",
        format!("[ {} 88 ]", t(Text::Source)),
        TextStyle::Name,
        Budget::Dip(SOURCE_NAME_W),
    ));
    out.push(place(
        "unused bus",
        t(Text::Unused),
        TextStyle::Body,
        Budget::LikeTurkish,
    ));
    out
}

fn output_places(lang: Lang) -> Vec<Place> {
    let mut out = Vec::new();
    for kind in KINDS {
        let caption = kind.caption(lang);
        out.push(place(
            &format!("default caption {kind:?}"),
            format!("{} \u{b7} {caption}", text(Text::DefaultCaption, lang)),
            TextStyle::Sub,
            Budget::Dip(OUTPUT_NAME_W),
        ));
        out.push(place(
            &format!("other caption {kind:?}"),
            caption,
            TextStyle::Sub,
            Budget::Dip(OUTPUT_NAME_W),
        ));
    }
    out
}

/// The muted and off words in each slot they are drawn in.
fn muted_places(lang: Lang) -> Vec<Place> {
    let muted = text(Text::Muted, lang);
    let off = text(Text::Off, lang);
    let label = TextStyle::Label;
    let value = TextStyle::Value;
    // The output row sizes its value box from the character count, `VALUE_CHAR_W` each.
    #[allow(clippy::cast_precision_loss)]
    let row_box = muted.chars().count() as f64 * VALUE_CHAR_W + VALUE_SLACK;
    vec![
        place("cut route muted", muted, label, Budget::Dip(CUT_LABEL_W)),
        place("output row muted", muted, label, Budget::Dip(row_box)),
        place(
            "default output muted",
            muted,
            label,
            Budget::Dip(VALUE_BOX_W),
        ),
        place("microphone off", off, label, Budget::Dip(CUT_TO - CUT_FROM)),
        // The OSD draws its value slot in `TextStyle::Value` (`plith-osd`, `paint.rs`).
        place("osd muted", muted, value, Budget::Dip(OSD_VALUE_W)),
        place("osd off", off, value, Budget::Dip(OSD_VALUE_W)),
    ]
}

fn pill_places(lang: Lang) -> Vec<Place> {
    let off = text(Text::Off, lang);
    let value = TextStyle::Value;
    let mut out = vec![
        place(
            "pill muted word",
            off,
            TextStyle::Label,
            Budget::Dip(pill::VALUE_W),
        ),
        place("pill off", off, value, Budget::Dip(TURN_W)),
        place(
            "full percent",
            percent(100, lang),
            value,
            Budget::Dip(TURN_W),
        ),
        place(
            "system percent",
            percent(100, lang),
            TextStyle::Clock,
            Budget::Dip(VALUE_BOX_W),
        ),
    ];
    for kind in KINDS {
        let words = device_kind(kind, lang);
        if let Some(s) = sound_words(Some(100), false, words, off, lang) {
            out.push(place(
                &format!("pill sound {kind:?}"),
                s.text,
                value,
                Budget::Dip(TURN_W - GLYPH_ROOM),
            ));
        }
    }
    out
}

fn welcome_places(lang: Lang) -> Vec<Place> {
    let label = TextStyle::Label;
    let line = Budget::Dip(LINE_W);
    let t = |id| text(id, lang);
    let mut out = vec![
        place("welcome off", t(Text::Off), label, line),
        place(
            "welcome default output",
            t(Text::DefaultOutput),
            label,
            line,
        ),
        place("welcome earlier", t(Text::Earlier), label, line),
        place(
            "welcome nothing played",
            t(Text::NothingPlayed),
            label,
            Budget::Dip(RUN_LABEL_W),
        ),
        place(
            "welcome read",
            format!("{} 23:59 \u{b7} {}", t(Text::Read), t(Text::Old)),
            label,
            line,
        ),
        place(
            "welcome forecast",
            format!("23:00 \u{b7} {}", t(Text::Forecast)),
            label,
            line,
        ),
        place(
            "welcome songs",
            format!("23:00 \u{2013} 00:00 \u{b7} {}", songs(88, lang)),
            label,
            line,
        ),
    ];
    for sky in [Sky::Clear, Sky::Cloud, Sky::Rain, Sky::Snow] {
        let word = sky_word(sky, lang);
        out.push(place(
            &format!("welcome sky {sky:?}"),
            format!("{word} \u{b7} -40\u{b0}C \u{b7} {}", percent(88, lang)),
            TextStyle::Name,
            line,
        ));
        out.push(place(
            &format!("welcome turn {sky:?}"),
            format!("23:00 {word} \u{b7} -40\u{b0}C"),
            label,
            line,
        ));
    }
    for kind in KINDS {
        out.push(place(
            &format!("welcome sound {kind:?}"),
            format!("{} \u{b7} {}", percent(100, lang), device_kind(kind, lang)),
            label,
            line,
        ));
    }
    for weekday in 0..7 {
        for month in 1..=12 {
            out.push(place(
                &format!("welcome date {weekday} {month}"),
                date_short(weekday, 28, month, lang),
                label,
                Budget::Dip(DATE_W),
            ));
        }
    }
    out
}

/// Strings that must have glyphs in the bundled fonts, with the style each is set in: every
/// place, plus every table string in the two styles own copy uses.
#[must_use]
pub fn glyph_strings(lang: Lang) -> Vec<(String, TextStyle)> {
    let mut out: Vec<(String, TextStyle)> = places(lang)
        .into_iter()
        .map(|p| (p.text, p.style))
        .collect();
    for t in Text::ALL {
        let s = text(t, lang);
        out.push((s.to_owned(), TextStyle::Label));
        out.push((s.to_owned(), TextStyle::Name));
    }
    for n in [0, 1, 2, 5, 22] {
        out.push((songs(n, lang), TextStyle::Label));
    }
    out
}

#[cfg(test)]
mod tests;
