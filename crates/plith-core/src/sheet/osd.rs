//! The OSD section: the on-screen display and the Windows flyout.

use super::SheetFacts;
use super::controls::Page;
use super::layout::FIRST_ROW;
use crate::lang::sheet::SheetText as S;
use crate::settings::FlyoutPref;
use crate::settings::change::SettingChange;

pub(super) fn build(p: &mut Page, facts: &SheetFacts) {
    let s = &facts.settings;
    let mut t = FIRST_ROW;
    p.rule(t);

    let h = p.head(t, p.word(S::RowOsd), Some(p.word(S::SubOsd)), true);
    p.toggle(0, t, s.osd, SettingChange::Osd(!s.osd), true);
    t += h;
    p.rule(t);

    let hide = s.flyout == FlyoutPref::Hide;
    let to = if hide {
        FlyoutPref::Off
    } else {
        FlyoutPref::Hide
    };
    let h = p.head(
        t,
        p.word(S::RowHideWindows),
        Some(p.word(S::SubHideWindows)),
        s.osd,
    );
    p.toggle(1, t, hide, SettingChange::Flyout(to), s.osd);
    t += h;
    p.rule(t);
}
