//! Fit budgets of the installer copy, merged into `lang::fit::places`.
//!
//! The window is 640 wide. Step labels sit beside their ripple (`1 DOSYALAR`), button labels in
//! the 200 wide pill, leads and titles in the window less its margins.

use super::{SetupText, setup_text};
use crate::draw::TextStyle;
use crate::lang::Lang;
use crate::lang::fit::{Budget, Place};

/// Room of a step label with its number, from the layout (right of the ripple's end).
pub const STEP_W: f64 = 140.0;
/// Room of a label inside the 200 wide pill (the pill less its padding).
const BUTTON_W: f64 = 170.0;
/// Room of a check label or the close link.
const CHECK_W: f64 = 320.0;
/// Room of a lead, a sub line or a title (the window less 40 on each side).
const LINE_W: f64 = 560.0;

fn place(name: String, text: String, style: TextStyle, dip: f64) -> Place {
    Place {
        name,
        text,
        style,
        budget: Budget::Dip(dip),
    }
}

/// Every installer string with its style and room.
#[must_use]
pub fn places(lang: Lang) -> Vec<Place> {
    use SetupText as S;
    let mut out = Vec::new();
    for (n, id) in [(1, S::Step1), (2, S::Step2), (3, S::Step3)] {
        let text = format!("{n} {}", setup_text(id, lang));
        out.push(place(
            format!("setup {id:?}"),
            text,
            TextStyle::Step,
            STEP_W,
        ));
    }
    for id in S::ALL {
        let text = setup_text(id, lang).to_owned();
        let (style, dip) = match id {
            S::Step1 | S::Step2 | S::Step3 => continue,
            S::Install | S::Open | S::Update | S::Retry | S::Uninstall => {
                (TextStyle::Button, BUTTON_W)
            }
            S::Autostart | S::Close | S::DeleteData => (TextStyle::Label, CHECK_W),
            S::DoneTitle | S::UninstallDone => (TextStyle::Display, LINE_W),
            S::LeadStart
            | S::LeadRunning
            | S::ErrorTitle
            | S::NewerTitle
            | S::UpdateLead
            | S::UninstallLead => (TextStyle::Heading, LINE_W),
            S::SubUac | S::DoneSub => (TextStyle::Strong, LINE_W),
            S::UacCancelled => (TextStyle::Body, LINE_W),
        };
        if id == S::Close {
            // The same word also labels the pill of the newer and uninstalled screens.
            out.push(place(
                "setup Close pill".to_owned(),
                text.clone(),
                TextStyle::Button,
                BUTTON_W,
            ));
        }
        out.push(place(format!("setup {id:?}"), text, style, dip));
    }
    out
}
