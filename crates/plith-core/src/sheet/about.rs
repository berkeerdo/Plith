//! The About section: the app icon with name, version and build date, then the settings file
//! and the data sources.

use super::controls::{Page, rect};
use super::layout::{FIRST_ROW, R, X};
use super::{ICON_ART, ICON_DIP, SheetAction, SheetFacts};
use crate::draw::{Align, Ink, Node, TextStyle};
use crate::lang::sheet::{SheetText as S, fill};
use crate::update::UpdateRow;

/// Gap between the icon and the name.
const NAME_GAP: f64 = 20.0;
/// Room of the update state caption, left of its button.
const STATE_W: f64 = 260.0;
/// Gap between the header and the first row.
const HEADER_GAP: f64 = 24.0;

pub(super) fn build(p: &mut Page, facts: &SheetFacts) {
    header(p, facts);
    let mut t = FIRST_ROW + ICON_DIP + HEADER_GAP;
    p.rule(t);

    let h = p.head(t, p.word(S::RowFile), Some(&facts.settings_path), true);
    let open = p.word(S::WordOpen);
    p.button(1, 0, t, open, SheetAction::OpenFolder);
    t += h;
    p.rule(t);

    let h = p.head(t, p.word(S::RowUpdate), None, true);
    update_state(p, t, facts.update);
    t += h;
    p.rule(t);

    let h = p.head(t, p.word(S::RowData), Some(p.word(S::SubData)), true);
    t += h;
    p.rule(t);
}

/// The state of the update row and its button: `DENETLE` when a check can run, `KUR` when a
/// version is ready, no button while a check or a download runs. Before the first check there is
/// no state text at all.
fn update_state(p: &mut Page, t: f64, row: UpdateRow) {
    let (state, ink, button) = match row {
        // Nothing is known yet: no state is claimed, only a check is offered.
        UpdateRow::Unknown => (String::new(), Ink::Muted, Some(false)),
        UpdateRow::Current => (
            p.word(S::StateUpdateCurrent).to_owned(),
            Ink::Muted,
            Some(false),
        ),
        UpdateRow::Checking => (p.word(S::StateUpdateChecking).to_owned(), Ink::Muted, None),
        UpdateRow::Ready(version) => (
            fill(p.word(S::StateUpdateReady), "version", &version.to_string()),
            Ink::Ink,
            Some(true),
        ),
        UpdateRow::Downloading(percent) => (
            fill(
                p.word(S::StateUpdateDownloading),
                "percent",
                &percent.to_string(),
            ),
            Ink::Muted,
            None,
        ),
        UpdateRow::BadSignature => (
            p.word(S::StateUpdateBadSignature).to_owned(),
            Ink::Ink,
            Some(false),
        ),
        UpdateRow::Failed => (
            p.word(S::StateUpdateFailed).to_owned(),
            Ink::Ink,
            Some(false),
        ),
    };
    let edge = match button {
        Some(true) => {
            let word = p.word(S::WordInstall);
            R - p.button(2, 0, t, word, SheetAction::UpdateInstall) - 8.0
        }
        Some(false) => {
            let word = p.word(S::WordCheck);
            R - p.button(2, 0, t, word, SheetAction::UpdateCheck) - 8.0
        }
        None => R - 4.0,
    };
    if state.is_empty() {
        return;
    }
    p.text(
        rect(edge - STATE_W, t + 9.0, STATE_W, 14.0),
        &state,
        TextStyle::Label,
        ink,
        Align::End,
    );
}

/// The icon, the brand name (never upper-cased by language rules) and version with date.
fn header(p: &mut Page, facts: &SheetFacts) {
    p.nodes.push(Node::Image {
        rect: rect(X, FIRST_ROW, ICON_DIP, ICON_DIP),
        art: ICON_ART,
    });
    let x = X + ICON_DIP + NAME_GAP;
    p.text(
        rect(x, FIRST_ROW + 14.0, R - x, 36.0),
        "Plith",
        TextStyle::Hero,
        Ink::Ink,
        Align::Start,
    );
    p.text(
        rect(x, FIRST_ROW + 58.0, R - x, 14.0),
        &format!("{}  {}", facts.version, facts.build_date),
        TextStyle::Value,
        Ink::Muted,
        Align::Start,
    );
}
