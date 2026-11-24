//! The General section: language and the page the island opens on.

use super::controls::{Page, word};
use super::layout::FIRST_ROW;
use super::{SheetAction, SheetFacts};
use crate::lang::Lang;
use crate::lang::sheet::{SheetText as S, lang_name};
use crate::settings::change::SettingChange;
use crate::settings::{LangChoice, OpenOn};

pub(super) fn build(p: &mut Page, facts: &SheetFacts) {
    let s = &facts.settings;
    let mut t = FIRST_ROW;
    p.rule(t);

    let mut words = vec![word(
        p.word(S::WordAuto),
        s.lang == LangChoice::Auto,
        SettingChange::Lang(LangChoice::Auto),
    )];
    for lang in Lang::ALL {
        let choice = LangChoice::Code(lang.code().to_owned());
        words.push(word(
            lang_name(lang),
            s.lang == choice,
            SettingChange::Lang(choice),
        ));
    }
    let h = p.head(
        t,
        p.word(S::RowLanguage),
        Some(p.word(S::SubLanguage)),
        true,
    );
    t += h + p.choice(0, t, words, true);
    p.rule(t);

    let words = vec![
        word(
            p.word(S::WordWelcome),
            s.open_on == OpenOn::Welcome,
            SettingChange::OpenOn(OpenOn::Welcome),
        ),
        word(
            p.word(S::WordLastPage),
            s.open_on == OpenOn::LastPage,
            SettingChange::OpenOn(OpenOn::LastPage),
        ),
    ];
    let h = p.head(t, p.word(S::RowOpens), Some(p.word(S::SubOpens)), true);
    t += h + p.choice(1, t, words, true);
    p.rule(t);

    let h = p.head(
        t,
        p.word(S::RowAutostart),
        Some(p.word(S::SubAutostart)),
        true,
    );
    let to = SheetAction::Autostart(!facts.autostart);
    p.toggle_to(2, t, facts.autostart, to, true);
    t += h;
    p.rule(t);
}
