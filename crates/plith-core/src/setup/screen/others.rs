//! Screens with no comp, drawn by the start screen's rules: the lead at the start lead's place,
//! one pill where the start pill is, secondary actions as captions under it.

use super::{LEAD_Y, Page, START_BUTTON_Y, SetupAction, UNDER_Y, Version};
use crate::draw::{Ink, TextStyle};
use crate::lang::setup::SetupText as S;

const LEAD: (TextStyle, Ink) = (TextStyle::Heading, Ink::Ink);
/// The title of an end state: the same role as the done title.
const TITLE: (TextStyle, Ink) = (TextStyle::Display, Ink::Ink);
/// The line under the lead (a reason, a version).
const SUB_Y: f64 = LEAD_Y + 28.0;

/// A failed step: its ripple state, the step label, one line (the busy text, else the error
/// code unless it is 0), retry and close. `failure` is `(step, code, busy)`.
pub(super) fn error(p: &mut Page, own: Version, failure: (u8, u32, bool)) {
    let (step, code, busy) = failure;
    p.background(step.clamp(1, 3) - 1);
    p.chrome(own, true);
    p.steps(Some(step));
    p.centered(LEAD_Y, p.word(S::ErrorTitle), LEAD);
    let line = if busy {
        Some(p.word(S::Busy).to_owned())
    } else {
        (code != 0).then(|| format!("{} {code}", p.word(S::ErrorCode)))
    };
    if let Some(line) = line {
        p.centered(SUB_Y, &line, (TextStyle::Body, Ink::Muted));
    }
    p.button(START_BUTTON_Y, S::Retry, SetupAction::Retry);
    p.link(UNDER_Y, S::Close, SetupAction::Close);
}

/// A newer Plith is installed: the title, its version and a close pill.
pub(super) fn newer(p: &mut Page, own: Version, installed: Version) {
    p.background(3);
    p.chrome(own, true);
    p.centered(LEAD_Y, p.word(S::NewerTitle), LEAD);
    p.centered(
        SUB_Y,
        &installed.to_string(),
        (TextStyle::Strong, Ink::Muted),
    );
    p.button(START_BUTTON_Y, S::Close, SetupAction::Close);
}

/// The uninstall question: the icon whole, one pill and the delete my data check.
pub(super) fn uninstall_confirm(p: &mut Page, own: Version, delete_data: bool) {
    p.background(3);
    p.chrome(own, true);
    p.centered(LEAD_Y, p.word(S::UninstallLead), LEAD);
    p.button(START_BUTTON_Y, S::Uninstall, SetupAction::Uninstall);
    p.check(
        UNDER_Y,
        delete_data,
        S::DeleteData,
        SetupAction::ToggleDeleteData,
    );
}

/// After uninstalling: the flat fabric, a title and a close pill.
pub(super) fn uninstalled(p: &mut Page, own: Version) {
    p.background(0);
    p.chrome(own, true);
    p.centered(LEAD_Y, p.word(S::UninstallDone), TITLE);
    p.button(START_BUTTON_Y, S::Close, SetupAction::Close);
}
