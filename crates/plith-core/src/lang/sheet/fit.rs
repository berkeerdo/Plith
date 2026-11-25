//! Fit budgets of the settings window copy, merged into `lang::fit::places`.
//!
//! Row labels are set in `Name`, sub lines and notes in `Body` (mixed case, never upper-cased),
//! captions and choice words in `Label`.

use super::{SheetText, fill, sheet_text, theme_name};
use crate::draw::TextStyle;
use crate::lang::fit::{Budget, Place};
use crate::lang::{Lang, Text, text};
use crate::theme::Theme;

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
/// Room of the `all settings` button word and the update buttons.
const BUTTON_W: f64 = 150.0;
/// Room of the update state caption, as laid out in the About section.
const STATE_W: f64 = 260.0;
/// Room of a sub line in a row with no control on its right (the data row).
const WIDE_W: f64 = 560.0;

fn place(id: SheetText, lang: Lang, style: TextStyle, dip: f64) -> Place {
    Place {
        name: format!("sheet {id:?}"),
        text: sheet_text(id, lang).to_owned(),
        style,
        budget: Budget::Dip(dip),
    }
}

/// The six look names under the preview cells.
fn look_places(lang: Lang) -> impl Iterator<Item = Place> {
    [
        ("sheet look karot", Text::LookKarot),
        ("sheet look clock", Text::LookClock),
        ("sheet look playing", Text::LookPlaying),
        ("sheet look seal", Text::LookSeal),
        ("sheet look plain", Text::LookPlain),
        ("sheet look hidden", Text::LookHidden),
    ]
    .into_iter()
    .map(move |(name, id)| Place {
        name: name.to_owned(),
        text: text(id, lang).to_owned(),
        style: TextStyle::Label,
        budget: Budget::Dip(LOOK_W),
    })
}

/// The four theme names as choice words; they read the same in every language.
fn theme_places() -> impl Iterator<Item = Place> {
    Theme::ALL.into_iter().map(|theme| Place {
        name: format!("sheet theme {}", theme.key()),
        text: theme_name(theme).to_owned(),
        style: TextStyle::Label,
        budget: Budget::Dip(WORD_W),
    })
}

/// The update state captions, which hold a placeholder the code fills.
fn is_state(id: SheetText) -> bool {
    matches!(
        id,
        SheetText::StateUpdateCurrent
            | SheetText::StateUpdateReady
            | SheetText::StateUpdateChecking
            | SheetText::StateUpdateDownloading
            | SheetText::StateUpdateBadSignature
            | SheetText::StateUpdateFailed
    )
}

/// The update state captions with the widest value the code puts in.
fn state_places(lang: Lang) -> impl Iterator<Item = Place> {
    SheetText::ALL
        .into_iter()
        .filter(|id| is_state(*id))
        .map(move |id| {
            let mut shown = place(id, lang, TextStyle::Label, STATE_W);
            shown.text = fill(
                &fill(&shown.text, "version", "100.100.100"),
                "percent",
                "100",
            );
            shown
        })
}

/// Every settings window string with its style and room. The window title is not budgeted.
#[must_use]
pub fn places(lang: Lang) -> Vec<Place> {
    use SheetText as S;
    let mut out = Vec::new();
    for id in [
        S::SectionGeneral,
        S::SectionLook,
        S::SectionIdle,
        S::SectionOsd,
        S::SectionAbout,
    ] {
        out.push(place(id, lang, TextStyle::Name, SECTION_W));
        let mut heading = place(id, lang, TextStyle::Heading, LINE_W);
        heading.name = format!("sheet heading {id:?}");
        out.push(heading);
    }
    for id in [
        S::RowLanguage,
        S::RowOpens,
        S::RowTheme,
        S::RowStrip,
        S::RowTopRight,
        S::RowPlayed,
        S::RowWeather,
        S::RowLocation,
        S::RowUnit,
        S::RowOsd,
        S::RowHideWindows,
        S::RowAutostart,
        S::RowVersion,
        S::RowFile,
        S::RowData,
        S::RowUpdate,
        S::RowUpdateCheck,
    ] {
        out.push(place(id, lang, TextStyle::Name, LABEL_W));
    }
    for id in [
        S::LineGeneral,
        S::LineLook,
        S::LineIdle,
        S::LineOsd,
        S::LineAbout,
        S::SubLanguage,
        S::SubOpens,
        S::SubTheme,
        S::PreviewNote,
        S::SubStrip,
        S::SubTopRight,
        S::SubPlayed,
        S::SubWeather,
        S::SubLocation,
        S::SubOsd,
        S::SubHideWindows,
        S::SubAutostart,
        S::SubUpdateCheck,
        S::SaveError,
    ] {
        out.push(place(id, lang, TextStyle::Body, LINE_W));
    }
    out.extend(look_places(lang));
    out.extend(theme_places());
    out.extend(state_places(lang));
    out.push(place(S::SubData, lang, TextStyle::Body, WIDE_W));
    for id in S::ALL {
        if !id.is_caption() || is_state(id) {
            continue;
        }
        let dip = match id {
            S::Caption | S::CaptionClosed => LINE_W,
            S::WordAllSettings | S::WordCheck | S::WordInstall => BUTTON_W,
            _ => WORD_W,
        };
        out.push(place(id, lang, TextStyle::Label, dip));
    }
    out
}
