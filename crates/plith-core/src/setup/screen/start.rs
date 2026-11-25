//! The start, update and declined-prompt screens.

use super::{LEAD_Y, Page, START_BUTTON_Y, SetupAction, UNDER_Y, Version};
use crate::draw::{Ink, TextStyle};
use crate::lang::setup::SetupText as S;

const LEAD: (TextStyle, Ink) = (TextStyle::Heading, Ink::Ink);

/// The start screen; `declined` adds the line about the declined Windows prompt.
pub(super) fn start(p: &mut Page, own: Version, declined: bool) {
    p.background(0);
    p.chrome(own, true);
    p.centered(LEAD_Y, p.word(S::LeadStart), LEAD);
    p.button(START_BUTTON_Y, S::Install, SetupAction::Install);
    if declined {
        p.centered(
            UNDER_Y,
            p.word(S::UacCancelled),
            (TextStyle::Body, Ink::Ink),
        );
    }
}

/// The update screen: the lead, `from → to` and one button.
pub(super) fn update(p: &mut Page, own: Version, from: Version, to: Version) {
    p.background(0);
    p.chrome(own, true);
    p.centered(LEAD_Y, p.word(S::UpdateLead), LEAD);
    p.centered(
        LEAD_Y + 28.0,
        &format!("{from} \u{2192} {to}"),
        (TextStyle::Strong, Ink::Muted),
    );
    p.button(START_BUTTON_Y, S::Update, SetupAction::Install);
}
