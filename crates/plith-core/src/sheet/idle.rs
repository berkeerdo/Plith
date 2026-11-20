//! The Idle section: the six look previews and the welcome view settings.

use super::controls::{Page, Word, rect, word};
use super::layout::X;
use super::{SheetAction, SheetFacts, SheetView};
use crate::draw::{Align, Ink, Node, StationMark, TextStyle};
use crate::lang::Text;
use crate::lang::sheet::{SheetText as S, sheet_text};
use crate::lang::text;
use crate::pages::idle_pill::rest_shape;
use crate::settings::change::SettingChange;
use crate::settings::{HeadSlot, IdleLook, LocationMode, RunSpan, Settings, TempUnit};

/// Scale of a preview inset: 200 x 32 becomes 90 x 14.4.
const PREVIEW_SCALE: f64 = 0.45;
/// The looks in preview order, with their names.
const LOOKS: [(IdleLook, Text); 6] = [
    (IdleLook::Karot, Text::LookKarot),
    (IdleLook::Clock, Text::LookClock),
    (IdleLook::Playing, Text::LookPlaying),
    (IdleLook::Seal, Text::LookSeal),
    (IdleLook::Plain, Text::LookPlain),
    (IdleLook::Hidden, Text::LookHidden),
];
const CELL_W: f64 = 96.0;
const CELL_H: f64 = 64.0;
const CELL_Y: f64 = 116.0;
/// Where the first rule of the rows sits, under the preview note.
const ROWS_Y: f64 = 226.0;

fn previews(p: &mut Page, facts: &SheetFacts) {
    p.text(
        rect(X, 94.0, 300.0, 12.0),
        p.word(S::CaptionClosed),
        TextStyle::Label,
        Ink::Faint,
        Align::Start,
    );
    let gap = (628.0 - 6.0 * CELL_W) / 5.0;
    for (i, (look, name)) in LOOKS.into_iter().enumerate() {
        #[allow(clippy::cast_precision_loss)] // six cells
        let x = X + i as f64 * (CELL_W + gap);
        let on = facts.settings.look == look;
        let cell = rect(x, CELL_Y, CELL_W, CELL_H);
        p.nodes.push(super::outline(
            cell,
            if on { Ink::Ink } else { Ink::Border },
        ));
        let shape = rest_shape(look);
        let (px, py) = (x + 3.0, CELL_Y + 12.0);
        if look == IdleLook::Hidden {
            p.stroke(vec![(px, py + 1.0), (px + 90.0, py + 1.0)], Ink::Faint);
        } else {
            p.nodes.push(Node::Fill {
                rect: rect(
                    px,
                    py,
                    shape.width * PREVIEW_SCALE,
                    shape.height * PREVIEW_SCALE,
                ),
                radius: shape.drawn_radius() * PREVIEW_SCALE,
                ink: Ink::Floor,
            });
        }
        p.nodes.push(Node::Inset {
            list: facts.previews[i].clone(),
            x: px,
            y: py,
            scale: PREVIEW_SCALE,
        });
        let mark = if on {
            StationMark::Filled
        } else {
            StationMark::Hollow
        };
        p.station(
            x + 12.0,
            CELL_Y + 52.0,
            mark,
            if on { Ink::Ink } else { Ink::Faint },
        );
        p.text(
            rect(x + 22.0, CELL_Y + 46.0, 70.0, 12.0),
            text(name, facts.lang),
            TextStyle::Label,
            if on { Ink::Ink } else { Ink::Faint },
            Align::Start,
        );
        #[allow(clippy::cast_possible_truncation)] // six cells
        let id = super::FocusId::Row(0, i as u8);
        p.targets.push(super::SheetTarget {
            rect: cell,
            id,
            action: SheetAction::Change(SettingChange::Look(look)),
        });
    }
    p.text(
        rect(X, 192.0, 600.0, 16.0),
        p.word(S::PreviewNote),
        TextStyle::Body,
        Ink::Faint,
        Align::Start,
    );
}

/// The span, head and unit choice rows, which have no sub line of their own besides the text.
fn choice_row(p: &mut Page, t: f64, row: u8, label: S, sub: Option<S>, words: Vec<Word>) -> f64 {
    let h = p.head(t, p.word(label), sub.map(|s| p.word(s)), true);
    let t = t + h + p.choice(row, t, words, true);
    p.rule(t);
    t
}

pub(super) fn build(p: &mut Page, facts: &SheetFacts, view: &SheetView) {
    let s = &facts.settings;
    previews(p, facts);
    let mut t = ROWS_Y;
    p.rule(t);

    let lang = p.lang;
    let w = |id, on, c| word(sheet_text(id, lang), on, c);
    let words = vec![
        w(
            S::WordHours3,
            s.span == RunSpan::Three,
            SettingChange::Span(RunSpan::Three),
        ),
        w(
            S::WordHours6,
            s.span == RunSpan::Six,
            SettingChange::Span(RunSpan::Six),
        ),
        w(
            S::WordAllDay,
            s.span == RunSpan::Day,
            SettingChange::Span(RunSpan::Day),
        ),
    ];
    t = choice_row(p, t, 1, S::RowStrip, Some(S::SubStrip), words);

    let slots = [
        (S::WordAuto, HeadSlot::Auto),
        (S::WordWeather, HeadSlot::Weather),
        (S::WordSound, HeadSlot::Sound),
        (S::WordPlaying, HeadSlot::Playing),
        (S::WordEmpty, HeadSlot::Empty),
    ];
    let words = slots
        .into_iter()
        .map(|(id, slot)| w(id, s.head == slot, SettingChange::Head(slot)))
        .collect();
    t = choice_row(p, t, 2, S::RowTopRight, Some(S::SubTopRight), words);

    t = toggle_rows(p, s, t);
    t = location_row(p, facts, view, t);

    let words = vec![
        w(
            S::WordCelsius,
            s.unit == TempUnit::C,
            SettingChange::Unit(TempUnit::C),
        ),
        w(
            S::WordFahrenheit,
            s.unit == TempUnit::F,
            SettingChange::Unit(TempUnit::F),
        ),
    ];
    choice_row(p, t, 6, S::RowUnit, None, words);
}

/// The plays and weather toggle rows; returns the top of the next row.
fn toggle_rows(p: &mut Page, cfg: &Settings, mut t: f64) -> f64 {
    let rows = [
        (
            3,
            S::RowPlayed,
            S::SubPlayed,
            cfg.plays,
            SettingChange::Plays(!cfg.plays),
        ),
        (
            4,
            S::RowWeather,
            S::SubWeather,
            cfg.weather,
            SettingChange::Weather(!cfg.weather),
        ),
    ];
    for (row, label, sub, on, to) in rows {
        let height = p.head(t, p.word(label), Some(p.word(sub)), true);
        p.toggle(row, t, on, to, true);
        t += height;
        p.rule(t);
    }
    t
}

/// The location row; the city field gets its own line under it while the city mode is picked.
fn location_row(p: &mut Page, facts: &SheetFacts, view: &SheetView, mut t: f64) -> f64 {
    let s = &facts.settings;
    let modes = [
        (S::WordWindows, LocationMode::Windows),
        (S::WordIp, LocationMode::Ip),
        (S::WordCity, LocationMode::City),
    ];
    let words = modes
        .into_iter()
        .map(|(id, m)| word(p.word(id), s.location == m, SettingChange::Location(m)))
        .collect();
    let h = p.head(
        t,
        p.word(S::RowLocation),
        Some(p.word(S::SubLocation)),
        true,
    );
    let extra = p.choice(5, t, words, true);
    t += h + extra;
    if s.location == LocationMode::City {
        let editing = view.focus == Some(super::FocusId::Row(5, 3));
        let shown = if editing { &view.field.text } else { &s.city };
        p.field(5, 3, (X, t - 6.0), shown);
        t += 34.0;
    }
    p.rule(t);
    t
}
