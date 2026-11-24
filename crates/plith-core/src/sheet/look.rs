//! The Look section: the colour theme of the island and this window.

use super::SheetFacts;
use super::controls::{Page, word};
use super::layout::FIRST_ROW;
use crate::lang::sheet::{SheetText as S, theme_name};
use crate::settings::change::SettingChange;
use crate::theme::Theme;

pub(super) fn build(p: &mut Page, facts: &SheetFacts) {
    let mut t = FIRST_ROW;
    p.rule(t);

    let words = Theme::ALL
        .into_iter()
        .map(|theme| {
            word(
                theme_name(theme),
                facts.settings.theme == theme,
                SettingChange::Theme(theme),
            )
        })
        .collect();
    let h = p.head(t, p.word(S::RowTheme), Some(p.word(S::SubTheme)), true);
    t += h + p.choice(0, t, words, true);
    p.rule(t);
}

#[cfg(test)]
mod tests;
