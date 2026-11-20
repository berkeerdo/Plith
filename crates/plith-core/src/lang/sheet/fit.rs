//! Fit budgets of the settings window copy, merged into `lang::fit::places`.
//!
//! Row labels are set in `Name`, sub lines and notes in `Body` (mixed case, never upper-cased),
//! captions and choice words in `Label`.

use super::{SheetText, sheet_text};
use crate::draw::TextStyle;
use crate::lang::fit::{Budget, Place};
use crate::lang::{Lang, Text, text};

/// Room of a look name in a preview cell: 96 wide, minus the station and its gap, minus a margin.
const LOOK_W: f64 = 70.0;

/// Room of a row label.
const LABEL_W: f64 = 300.0;
/// Room of a sub line, a section line, a note or the save error.
const LINE_W: f64 = 400.0;
/// Room of one choice word in its cell.
const WORD_W: f64 = 90.0;
/// Room of a sidebar section name.
const SECTION_W: f64 = 120.0;
/// Room of the `all settings` button word.
const BUTTON_W: f64 = 150.0;

fn place(id: SheetText, lang: Lang, style: TextStyle, dip: f64) -> Place {
    Place {
        name: format!("sheet {id:?}"),
        text: sheet_text(id, lang).to_owned(),
        style,
        budget: Budget::Dip(dip),
    }
}

/// Every settings window string with its style and room. The window title is not budgeted.
#[must_use]
pub fn places(lang: Lang) -> Vec<Place> {
    use SheetText as S;
    let mut out = Vec::new();
    for id in [
        S::SectionGeneral,
        S::SectionIdle,
        S::SectionOsd,
        S::SectionAbout,
    ] {
        out.push(place(id, lang, TextStyle::Name, SECTION_W));
    }
    for id in [
        S::RowLanguage,
        S::RowOpens,
        S::RowStrip,
        S::RowTopRight,
        S::RowPlayed,
        S::RowWeather,
        S::RowLocation,
        S::RowUnit,
        S::RowOsd,
        S::RowHideWindows,
        S::RowVersion,
        S::RowFile,
        S::RowData,
    ] {
        out.push(place(id, lang, TextStyle::Name, LABEL_W));
    }
    for id in [
        S::LineGeneral,
        S::LineIdle,
        S::LineOsd,
        S::LineAbout,
        S::SubLanguage,
        S::SubOpens,
        S::PreviewNote,
        S::SubStrip,
        S::SubTopRight,
        S::SubPlayed,
        S::SubWeather,
        S::SubLocation,
        S::SubOsd,
        S::SubHideWindows,
        S::SubData,
        S::SaveError,
    ] {
        out.push(place(id, lang, TextStyle::Body, LINE_W));
    }
    for (name, id) in [
        ("sheet look karot", Text::LookKarot),
        ("sheet look clock", Text::LookClock),
        ("sheet look playing", Text::LookPlaying),
        ("sheet look seal", Text::LookSeal),
        ("sheet look plain", Text::LookPlain),
        ("sheet look hidden", Text::LookHidden),
    ] {
        out.push(Place {
            name: name.to_owned(),
            text: text(id, lang).to_owned(),
            style: TextStyle::Label,
            budget: Budget::Dip(LOOK_W),
        });
    }
    for id in S::ALL {
        if !id.is_caption() {
            continue;
        }
        let dip = match id {
            S::Caption | S::CaptionClosed => LINE_W,
            S::WordAllSettings => BUTTON_W,
            _ => WORD_W,
        };
        out.push(place(id, lang, TextStyle::Label, dip));
    }
    out
}
