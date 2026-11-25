//! The done screen: all three ripples pressed, the window is the icon.

use super::{Page, SetupAction, Version};
use crate::draw::{Ink, TextStyle};
use crate::lang::setup::SetupText as S;

pub(super) fn done(p: &mut Page, own: Version, autostart: bool) {
    p.background(3);
    p.chrome(own, true);
    p.centered(236.0, p.word(S::DoneTitle), (TextStyle::Heading, Ink::Ink));
    p.centered(274.0, p.word(S::DoneSub), (TextStyle::Strong, Ink::Ink));
    p.button(304.0, S::Open, SetupAction::Open);
    p.link(373.0, S::Close, SetupAction::Close);
    p.check(399.0, autostart, S::Autostart, SetupAction::ToggleAutostart);
}
