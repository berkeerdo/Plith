//! Geometry shared by every section and the chrome: sidebar, vertical rule and heading.

use super::controls::{Page, rect};
use super::{FocusId, SHEET_H, Section, SheetAction, SheetFacts, SheetTarget, SheetView};
use crate::draw::{Align, Ink, StationMark, TextStyle};
use crate::lang::sheet::{SheetText as S, sheet_text};

/// Left edge of the content column.
pub(super) const X: f64 = 184.0;
/// Right edge of the content column.
pub(super) const R: f64 = 812.0;
/// Width of the sidebar; its rule is drawn here.
const SIDEBAR_W: f64 = 156.0;
/// Height of a row with a sub line.
pub(super) const ROW_SUB: f64 = 52.0;
/// Height of a row without one.
pub(super) const ROW_PLAIN: f64 = 40.0;
/// Where the first row of a section with no header block starts.
pub(super) const FIRST_ROW: f64 = 92.0;

fn section_text(s: Section) -> S {
    match s {
        Section::General => S::SectionGeneral,
        Section::Look => S::SectionLook,
        Section::Idle => S::SectionIdle,
        Section::Osd => S::SectionOsd,
        Section::About => S::SectionAbout,
    }
}

fn line_text(s: Section) -> S {
    match s {
        Section::General => S::LineGeneral,
        Section::Look => S::LineLook,
        Section::Idle => S::LineIdle,
        Section::Osd => S::LineOsd,
        Section::About => S::LineAbout,
    }
}

/// The sidebar, its rule and the heading with the line (or the save error) under it.
pub(super) fn chrome(p: &mut Page, facts: &SheetFacts, view: &SheetView) {
    let lang = facts.lang;
    p.text(
        rect(24.0, 24.0, 120.0, 12.0),
        sheet_text(S::Caption, lang),
        TextStyle::Label,
        Ink::Faint,
        Align::Start,
    );
    for (i, s) in Section::ALL.into_iter().enumerate() {
        #[allow(clippy::cast_precision_loss)] // five sections
        let y = 58.0 + i as f64 * 32.0;
        let active = s == view.section;
        if active {
            p.station(32.0, y + 14.0, StationMark::Filled, Ink::Flow);
        }
        p.text(
            rect(44.0, y + 5.0, 108.0, 18.0),
            sheet_text(section_text(s), lang),
            TextStyle::Name,
            if active { Ink::Ink } else { Ink::Muted },
            Align::Start,
        );
        p.targets.push(SheetTarget {
            rect: rect(8.0, y, 140.0, 28.0),
            id: FocusId::Section(s),
            action: SheetAction::Section(s),
        });
    }
    p.stroke(vec![(SIDEBAR_W, 0.0), (SIDEBAR_W, SHEET_H)], Ink::Border);
    p.text(
        rect(X, 32.0, 500.0, 26.0),
        sheet_text(section_text(view.section), lang),
        TextStyle::Heading,
        Ink::Ink,
        Align::Start,
    );
    let (line, ink) = if facts.save_failed {
        (sheet_text(S::SaveError, lang), Ink::Muted)
    } else {
        (sheet_text(line_text(view.section), lang), Ink::Faint)
    };
    p.text(
        rect(X, 58.0, R - X, 16.0),
        line,
        TextStyle::Body,
        ink,
        Align::Start,
    );
}
