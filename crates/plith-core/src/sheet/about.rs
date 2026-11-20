//! The About section: version, the settings file and the data sources.

use super::controls::{Page, rect};
use super::layout::{FIRST_ROW, R};
use super::{SheetAction, SheetFacts};
use crate::draw::{Align, Ink, TextStyle};
use crate::lang::sheet::SheetText as S;

pub(super) fn build(p: &mut Page, facts: &SheetFacts) {
    let mut t = FIRST_ROW;
    p.rule(t);

    let h = p.head(t, p.word(S::RowVersion), None, true);
    p.text(
        rect(R - 200.0, t + 9.0, 200.0, 14.0),
        &format!("{}  {}", facts.version, facts.build_date),
        TextStyle::Value,
        Ink::Muted,
        Align::End,
    );
    t += h;
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
