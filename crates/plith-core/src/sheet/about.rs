//! The About section: the app icon with name, version and build date, then the settings file
//! and the data sources.

use super::controls::{Page, rect};
use super::layout::{FIRST_ROW, R, X};
use super::{ICON_ART, ICON_DIP, SheetAction, SheetFacts};
use crate::draw::{Align, Ink, Node, TextStyle};
use crate::lang::sheet::SheetText as S;

/// Gap between the icon and the name.
const NAME_GAP: f64 = 20.0;
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

    let h = p.head(t, p.word(S::RowData), Some(p.word(S::SubData)), true);
    t += h;
    p.rule(t);
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
