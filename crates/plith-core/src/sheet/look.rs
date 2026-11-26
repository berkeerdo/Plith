//! The Look section: the colour theme of the island and this window.

use super::SheetFacts;
use super::controls::{Page, word};
use super::layout::FIRST_ROW;
use crate::lang::sheet::SheetText as S;
use crate::settings::change::SettingChange;

pub(super) fn build(p: &mut Page, facts: &SheetFacts) {
    let mut t = FIRST_ROW;
    p.rule(t);

    let words = facts
        .themes
        .iter()
        .map(|card| {
            word(
                card.name.as_str(),
                facts.settings.theme == card.id,
                SettingChange::Theme(card.id.clone()),
            )
        })
        .collect();
    let h = p.head(t, p.word(S::RowTheme), Some(p.word(S::SubTheme)), true);
    t += h + p.choice(0, t, words, true);
    p.rule(t);
}

#[cfg(test)]
mod tests;
