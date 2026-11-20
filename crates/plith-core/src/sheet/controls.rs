//! The page builder and the four controls: choice group, toggle, text field and button.
//!
//! A row starts at its top rule `t`: the label sits 8 below it, the sub line 27 below, the control
//! on the label line. A dead control (`live` false) is drawn in the faint ink and gets no target.

use super::layout::{R, ROW_PLAIN, ROW_SUB, X};
use super::{FocusId, SheetAction, SheetTarget};
use crate::draw::{Align, Ink, Node, Rect, StationMark, TextStyle, Trim};
use crate::lang::Lang;
use crate::lang::sheet::{SheetText as S, sheet_text};
use crate::settings::change::SettingChange;
use std::sync::Arc;

/// Room of a choice group before it wraps.
const CHOICE_W: f64 = 420.0;
/// Advance of one mono caption character, with its tracking.
const ADVANCE: f64 = 7.4;
/// Padding inside a choice word's cell.
const WORD_PAD: f64 = 8.0;
/// Gap between choice words.
const WORD_GAP: f64 = 10.0;
/// Distance between wrapped lines of a choice group.
const LINE_STEP: f64 = 40.0;

/// A rectangle from its parts.
pub(super) fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
    Rect { x, y, w, h }
}

/// One word of a choice group.
pub(super) struct Word {
    /// The caption.
    pub text: &'static str,
    /// Whether it is the picked one.
    pub on: bool,
    /// What a click does.
    pub action: SheetAction,
}

/// What is drawn inside a focused text field besides its text, as offsets from the text start.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct FieldMark {
    /// Width of the selection fill behind the text.
    pub select_w: Option<f64>,
    /// Where the caret is.
    pub caret_x: Option<f64>,
}

/// The nodes and targets built so far.
pub(super) struct Page {
    pub nodes: Vec<Node>,
    pub targets: Vec<SheetTarget>,
    pub lang: Lang,
}

impl Page {
    pub fn new(lang: Lang) -> Self {
        Self {
            nodes: Vec::new(),
            targets: Vec::new(),
            lang,
        }
    }

    pub fn text(&mut self, r: Rect, s: &str, style: TextStyle, ink: Ink, align: Align) {
        self.push_text(r, s, (style, ink), (align, Trim::End));
    }

    fn push_text(&mut self, r: Rect, s: &str, look: (TextStyle, Ink), place: (Align, Trim)) {
        self.nodes.push(Node::Text {
            rect: r,
            text: Arc::from(s),
            style: look.0,
            ink: look.1,
            align: place.0,
            trim: place.1,
        });
    }

    pub fn stroke(&mut self, points: Vec<(f64, f64)>, ink: Ink) {
        self.nodes.push(Node::Stroke {
            points,
            width: 1.0,
            ink,
            dashed: false,
        });
    }

    pub fn station(&mut self, x: f64, y: f64, mark: StationMark, ink: Ink) {
        self.nodes.push(Node::Station { x, y, mark, ink });
    }

    pub fn word(&self, id: S) -> &'static str {
        sheet_text(id, self.lang)
    }

    /// A rule across the content column.
    pub fn rule(&mut self, y: f64) {
        self.stroke(vec![(X, y), (R, y)], Ink::Border);
    }

    /// The label and optional sub line of the row at `t`; returns the row height.
    pub fn head(&mut self, t: f64, label: &str, sub: Option<&str>, live: bool) -> f64 {
        let ink = if live { Ink::Ink } else { Ink::Faint };
        self.text(
            rect(X, t + 8.0, 300.0, 18.0),
            label,
            TextStyle::Name,
            ink,
            Align::Start,
        );
        let Some(sub) = sub else {
            return ROW_PLAIN;
        };
        self.push_text(
            rect(X, t + 27.0, 560.0, 16.0),
            sub,
            (TextStyle::Body, Ink::Faint),
            (Align::Start, Trim::Middle),
        );
        ROW_SUB
    }

    fn target(&mut self, r: Rect, id: FocusId, action: SheetAction) {
        self.targets.push(SheetTarget {
            rect: r,
            id,
            action,
        });
    }

    /// A choice group right-aligned at the content edge, wrapped onto more lines when it does
    /// not fit. Returns the extra height of the wrapped lines.
    #[allow(clippy::cast_precision_loss)] // a handful of lines
    pub fn choice(&mut self, row: u8, t: f64, words: Vec<Word>, live: bool) -> f64 {
        let widths: Vec<f64> = words.iter().map(|w| cell_w(w.text)).collect();
        let lines = wrap(&widths);
        let mut index = 0u8;
        let mut words = words.into_iter();
        for (n, line) in lines.iter().enumerate() {
            #[allow(clippy::cast_precision_loss)] // a handful of words
            let total: f64 =
                line.iter().sum::<f64>() + WORD_GAP * line.len().saturating_sub(1) as f64;
            #[allow(clippy::cast_precision_loss)] // a handful of lines
            let dy = n as f64 * LINE_STEP;
            let mut x = R - total;
            for &w in line {
                let word = words.next().expect("one width per word");
                self.choice_word(row, index, (x, t + dy, w), &word, live);
                index += 1;
                x += w + WORD_GAP;
            }
        }
        (lines.len() - 1) as f64 * LINE_STEP
    }

    fn choice_word(&mut self, row: u8, ctl: u8, at: (f64, f64, f64), word: &Word, live: bool) {
        let (x, t, w) = at;
        let on = word.on && live;
        self.text(
            rect(x, t + 9.0, w - 4.0, 14.0),
            word.text,
            TextStyle::Label,
            if on { Ink::Ink } else { Ink::Faint },
            Align::End,
        );
        if on {
            self.stroke(vec![(x + 4.0, t + 26.0), (x + w - 4.0, t + 26.0)], Ink::Ink);
        }
        if live {
            self.target(
                rect(x, t + 4.0, w, 24.0),
                FocusId::Row(row, ctl),
                word.action.clone(),
            );
        }
    }

    /// A toggle: the word, then a track with two stations; filled on the right is on, both hollow is off.
    pub fn toggle(&mut self, row: u8, t: f64, on: bool, to: SettingChange, live: bool) {
        let y = t + 16.0;
        let (ink, hollow) = if live {
            (Ink::Ink, Ink::Faint)
        } else {
            (Ink::Faint, Ink::Faint)
        };
        self.stroke(vec![(R - 40.0, y), (R - 4.0, y)], Ink::Track);
        if on {
            self.station(R - 40.0, y, StationMark::Hollow, hollow);
            self.station(R - 4.0, y, StationMark::Filled, ink);
        } else {
            self.station(R - 40.0, y, StationMark::Hollow, ink);
            self.station(R - 4.0, y, StationMark::Hollow, hollow);
        }
        let word = self.word(if on { S::WordOn } else { S::WordOff });
        self.text(
            rect(R - 134.0, t + 9.0, 84.0, 14.0),
            word,
            TextStyle::Label,
            if on { ink } else { hollow },
            Align::End,
        );
        if live {
            let r = rect(R - 134.0, t + 4.0, 134.0, 24.0);
            self.target(r, FocusId::Row(row, 0), SheetAction::Change(to));
        }
    }

    /// A button word at the right edge.
    pub fn button(&mut self, row: u8, ctl: u8, t: f64, word: &str, action: SheetAction) {
        self.text(
            rect(R - 64.0, t + 9.0, 60.0, 14.0),
            word,
            TextStyle::Label,
            Ink::Ink,
            Align::End,
        );
        self.stroke(vec![(R - 60.0, t + 26.0), (R - 4.0, t + 26.0)], Ink::Muted);
        self.target(
            rect(R - 68.0, t + 4.0, 68.0, 24.0),
            FocusId::Row(row, ctl),
            action,
        );
    }

    /// A one line text field box at `(x, y)`, 240 wide, showing `shown`, with the selection
    /// behind the text and the caret over it when `mark` has them.
    pub fn field(&mut self, id: (u8, u8), at: (f64, f64), shown: &str, mark: FieldMark) {
        let (x, y) = at;
        let r = rect(x, y, 240.0, 26.0);
        self.nodes.push(Node::Fill {
            rect: r,
            radius: 0.0,
            ink: Ink::Floor,
        });
        self.nodes.push(super::outline(r, Ink::Border));
        if let Some(w) = mark.select_w {
            self.nodes.push(Node::Fill {
                rect: rect(x + 10.0, y + 4.0, w.clamp(0.0, 220.0), 18.0),
                radius: 0.0,
                ink: Ink::Track,
            });
        }
        self.text(
            rect(x + 10.0, y + 5.0, 220.0, 16.0),
            shown,
            TextStyle::Body,
            Ink::Ink,
            Align::Start,
        );
        if let Some(cx) = mark.caret_x {
            self.nodes.push(Node::Fill {
                rect: rect(x + 10.0 + cx.clamp(0.0, 220.0), y + 5.0, 1.0, 16.0),
                radius: 0.0,
                ink: Ink::Ink,
            });
        }
        self.target(r, FocusId::Row(id.0, id.1), SheetAction::FocusField);
    }
}

/// Width of one choice word's cell.
#[allow(clippy::cast_precision_loss)] // a few characters
fn cell_w(text: &str) -> f64 {
    text.chars().count() as f64 * ADVANCE + WORD_PAD
}

/// Groups cell widths into lines of at most `CHOICE_W` (a lone wide cell still gets a line).
fn wrap(widths: &[f64]) -> Vec<Vec<f64>> {
    let mut lines: Vec<Vec<f64>> = vec![Vec::new()];
    let mut used = 0.0;
    for &w in widths {
        let line = lines.last_mut().expect("never empty");
        let need = if line.is_empty() {
            w
        } else {
            used + WORD_GAP + w
        };
        if !line.is_empty() && need > CHOICE_W {
            lines.push(vec![w]);
            used = w;
        } else {
            line.push(w);
            used = need;
        }
    }
    lines
}

/// A choice word that changes a setting.
pub(super) fn word(text: &'static str, on: bool, change: SettingChange) -> Word {
    Word {
        text,
        on,
        action: SheetAction::Change(change),
    }
}
