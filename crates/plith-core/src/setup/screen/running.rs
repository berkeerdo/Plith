//! The running screens: no close mark while work is going on.

use super::{Page, Version};
use crate::draw::{Ink, TextStyle};
use crate::lang::setup::SetupText as S;

/// The lead's centre line while running.
const LEAD_Y: f64 = 262.0;
/// The sub line's centre line while running.
const SUB_Y: f64 = 292.0;
/// Steps finished before the Windows prompt for the helper has been answered.
const BEFORE_PROMPT: u8 = 2;

/// Installing: the three steps with `done` pressed; the sub line names the Windows prompt until
/// the helper is trusted.
pub(super) fn running(p: &mut Page, own: Version, done: u8) {
    p.background(done);
    p.chrome(own, false);
    p.steps(None);
    p.centered(
        LEAD_Y,
        p.word(S::LeadRunning),
        (TextStyle::Heading, Ink::Ink),
    );
    if done < BEFORE_PROMPT {
        p.centered(SUB_Y, p.word(S::SubUac), (TextStyle::Strong, Ink::Ink));
    }
}

/// Uninstalling: the install backgrounds in reverse, the ripples release one by one.
pub(super) fn uninstalling(p: &mut Page, own: Version, done: u8) {
    p.background(3 - done.min(3));
    p.chrome(own, false);
    p.centered(
        LEAD_Y,
        p.word(S::UninstallLead),
        (TextStyle::Heading, Ink::Ink),
    );
}
