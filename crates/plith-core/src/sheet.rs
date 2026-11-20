//! The settings window content as a display list with click targets, 840 x 640 DIP.
//!
//! The layout follows the drawn Idle section in `docs/design/r7/karot.html`. Everything is pure:
//! the window feeds facts and view state in and gets nodes and targets back.

use crate::draw::{DisplayList, Ink, Node, Rect};
use crate::lang::Lang;
use crate::settings::Settings;
use crate::settings::change::SettingChange;
use std::sync::Arc;

mod about;
mod controls;
mod field;
mod general;
mod idle;
mod keys;
mod layout;
mod osd;

pub use field::{FIELD_MAX, FieldOutcome, TextField};
pub use keys::{KeyOutcome, SheetKey, commit_field, focus_field, on_key};

#[cfg(test)]
mod tests;

/// Width of the sheet in DIP.
pub const SHEET_W: f64 = 840.0;
/// Height of the sheet in DIP.
pub const SHEET_H: f64 = 640.0;

/// A section of the sidebar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Section {
    /// Language and the opening page.
    General,
    /// The resting island and the welcome view.
    #[default]
    Idle,
    /// The volume and brightness display.
    Osd,
    /// Version and files.
    About,
}

impl Section {
    /// Every section in sidebar order.
    pub const ALL: [Section; 4] = [
        Section::General,
        Section::Idle,
        Section::Osd,
        Section::About,
    ];
}

/// What the sheet needs to know, besides the view state.
#[derive(Debug, Clone)]
pub struct SheetFacts {
    /// The current settings.
    pub settings: Settings,
    /// The interface language in use.
    pub lang: Lang,
    /// The six resting pills in `IdleLook` order (Karot, Clock, Playing, Seal, Plain, Hidden).
    pub previews: [Arc<DisplayList>; 6],
    /// The version shown in the About section.
    pub version: &'static str,
    /// The path of the settings file.
    pub settings_path: String,
    /// Whether the last save failed.
    pub save_failed: bool,
}

/// Where keyboard focus or the pointer is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusId {
    /// A sidebar entry.
    Section(Section),
    /// A control: the row, then the control index inside the row.
    Row(u8, u8),
}

/// What the window keeps between builds.
#[derive(Debug, Clone, Default)]
pub struct SheetView {
    /// The section shown.
    pub section: Section,
    /// The control with keyboard focus.
    pub focus: Option<FocusId>,
    /// The control under the pointer.
    pub hover: Option<FocusId>,
    /// The city field being edited.
    pub field: TextField,
    /// Whether the caret is shown now. Core has no clock: the window flips it (about every half
    /// second while the field has focus) and resets it on a key.
    pub caret_on: bool,
    /// Width in DIP of the field text before the caret, measured by the window with the field's
    /// font (core has no text measurement).
    pub caret_x: f64,
    /// Width in DIP of the whole field text, for the selection fill; measured like `caret_x`.
    pub text_w: f64,
}

/// What a click does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SheetAction {
    /// Show a section.
    Section(Section),
    /// Change a setting.
    Change(SettingChange),
    /// Focus the city field.
    FocusField,
    /// Open the settings folder.
    OpenFolder,
}

/// A clickable rectangle.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetTarget {
    /// Where it is.
    pub rect: Rect,
    /// Its identity for focus and hover.
    pub id: FocusId,
    /// What a click does.
    pub action: SheetAction,
}

/// The first target containing the point, no slack.
#[must_use]
pub fn sheet_hit(targets: &[SheetTarget], x: f64, y: f64) -> Option<&SheetTarget> {
    targets.iter().find(|t| {
        x >= t.rect.x && x < t.rect.x + t.rect.w && y >= t.rect.y && y < t.rect.y + t.rect.h
    })
}

fn outline(r: Rect, ink: Ink) -> Node {
    let (x0, y0, x1, y1) = (r.x + 0.5, r.y + 0.5, r.x + r.w - 0.5, r.y + r.h - 0.5);
    Node::Stroke {
        points: vec![(x0, y0), (x1, y0), (x1, y1), (x0, y1), (x0, y0)],
        width: 1.0,
        ink,
        dashed: false,
    }
}

/// Builds the sheet for the view's section and the click targets over it.
#[must_use]
pub fn sheet_build(facts: &SheetFacts, view: &SheetView) -> (DisplayList, Vec<SheetTarget>) {
    let mut p = controls::Page::new(facts.lang);
    layout::chrome(&mut p, facts, view);
    match view.section {
        Section::General => general::build(&mut p, facts),
        Section::Idle => idle::build(&mut p, facts, view),
        Section::Osd => osd::build(&mut p, facts),
        Section::About => about::build(&mut p, facts),
    }
    let (mut nodes, targets) = (p.nodes, p.targets);
    for t in &targets {
        if view.focus == Some(t.id) {
            nodes.push(outline(t.rect, Ink::Ink));
        } else if view.hover == Some(t.id) {
            nodes.push(outline(t.rect, Ink::Border));
        }
    }
    let list = DisplayList {
        nodes,
        size: (SHEET_W, SHEET_H),
    };
    (list, targets)
}
