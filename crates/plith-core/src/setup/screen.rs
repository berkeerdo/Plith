//! The installer, updater and uninstaller screens as display lists (direction "A Halka").
//!
//! The 640 x 420 window is the icon's lime speaker fabric: raster backgrounds (`BG_ART`), the
//! glossy black pill (`BUTTON_ART`) and nothing else drawn as image. Code draws the text, the
//! button label in lime, the checkbox and the close mark. The three ripples are the three
//! install steps; a finished step's ripple is pressed, which is the background state 0 to 3.
//! Layout numbers are the README's "Layout used on the page" in
//! `docs/design/identity/r3/installer/`. A text line is placed by its vertical centre.

use super::Version;
use crate::draw::{Align, ArtId, DisplayList, Ink, Node, Rect, TextStyle, Trim};
use crate::lang::Lang;
use crate::lang::setup::{SetupText, setup_text};
use std::sync::Arc;

mod done;
mod others;
mod running;
mod start;

#[cfg(test)]
mod tests;

/// Window width in DIP.
pub const SETUP_W: f64 = 640.0;
/// Window height in DIP.
pub const SETUP_H: f64 = 420.0;
/// The four fabric backgrounds, `"SET0"` to `"SET3"` in ASCII: no ripple pressed to all three
/// pressed. Each is drawn over the whole window.
pub const BG_ART: [ArtId; 4] = [
    ArtId(0x5345_5430),
    ArtId(0x5345_5431),
    ArtId(0x5345_5432),
    ArtId(0x5345_5433),
];
/// The glossy black pill, `"SETB"` in ASCII, drawn into a 200 x 55 rectangle.
pub const BUTTON_ART: ArtId = ArtId(0x5345_5442);

/// The window area above the ripples' end that drags the window.
const DRAG_H: f64 = 215.0;
/// Pill button width.
const BUTTON_W: f64 = 200.0;
/// Pill button height.
const BUTTON_H: f64 = 55.0;
/// Where the pill starts: centred.
const BUTTON_X: f64 = 220.0;
/// Side margin of a centred line.
const MARGIN: f64 = 40.0;
/// Advance of one mono caption character with its tracking, for centring and underlines.
const ADVANCE: f64 = 7.4;
/// Checkbox edge.
const CHECK: f64 = 16.0;
/// Gap between a checkbox and its label.
const CHECK_GAP: f64 = 10.0;
/// Step label places: left edge and centre line of steps 1 to 3.
const STEP_AT: [(f64, f64); 3] = [(430.0, 38.0), (468.0, 68.0), (496.0, 108.0)];
/// Start lead and the lead of the screens that have no comp.
const LEAD_Y: f64 = 250.0;
/// Start button top.
const START_BUTTON_Y: f64 = 298.0;
/// The line under the start button (declined prompt, secondary link, check).
const UNDER_Y: f64 = 380.0;

/// A screen of the setup program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Screen {
    /// The first screen: one button.
    Start,
    /// Installing; `done` steps (0 to 3) are finished.
    Running {
        /// Finished steps.
        done: u8,
    },
    /// Installed.
    Done {
        /// Whether the start with Windows check is ticked.
        autostart: bool,
    },
    /// A step failed.
    Error {
        /// The failed step, 1 to 3.
        step: u8,
        /// What the worker reported, drawn as given.
        reason: String,
    },
    /// A newer Plith is already installed.
    Newer {
        /// The installed version.
        installed: Version,
    },
    /// An update over an older install.
    Update {
        /// The installed version.
        from: Version,
        /// The version this setup carries.
        to: Version,
    },
    /// The uninstall question.
    UninstallConfirm {
        /// Whether the user's data goes too.
        delete_data: bool,
    },
    /// Uninstalling; `done` steps (0 to 3) are finished.
    Uninstalling {
        /// Finished steps.
        done: u8,
    },
    /// Uninstalled.
    Uninstalled,
    /// The start screen after the user declined the Windows prompt.
    UacCancelled,
}

/// What a click on a target does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupAction {
    /// Start installing or updating.
    Install,
    /// Open Plith.
    Open,
    /// Close the window.
    Close,
    /// Flip the start with Windows check.
    ToggleAutostart,
    /// Flip the delete my data check.
    ToggleDeleteData,
    /// Try the failed install again.
    Retry,
    /// Start uninstalling.
    Uninstall,
    /// Drag the window.
    Drag,
}

/// A clickable area.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SetupTarget {
    /// The area in window DIP.
    pub rect: Rect,
    /// What a click does.
    pub action: SetupAction,
}

/// The display list and click targets of `screen`. `own` is this setup's version, shown at the
/// top left. Targets are ordered by priority; the drag area is last.
#[must_use]
pub fn setup_build(screen: &Screen, own: Version, lang: Lang) -> (DisplayList, Vec<SetupTarget>) {
    let mut p = Page::new(lang);
    match screen {
        Screen::Start => start::start(&mut p, own, false),
        Screen::UacCancelled => start::start(&mut p, own, true),
        Screen::Update { from, to } => start::update(&mut p, own, *from, *to),
        Screen::Running { done } => running::running(&mut p, own, *done),
        Screen::Uninstalling { done } => running::uninstalling(&mut p, own, *done),
        Screen::Done { autostart } => done::done(&mut p, own, *autostart),
        Screen::Error { step, reason } => others::error(&mut p, own, *step, reason),
        Screen::Newer { installed } => others::newer(&mut p, own, *installed),
        Screen::UninstallConfirm { delete_data } => {
            others::uninstall_confirm(&mut p, own, *delete_data);
        }
        Screen::Uninstalled => others::uninstalled(&mut p, own),
    }
    p.finish()
}

/// The action under the point, the first target that holds it.
#[must_use]
pub fn setup_hit(targets: &[SetupTarget], x: f64, y: f64) -> Option<SetupAction> {
    targets
        .iter()
        .find(|t| {
            x >= t.rect.x && x < t.rect.x + t.rect.w && y >= t.rect.y && y < t.rect.y + t.rect.h
        })
        .map(|t| t.action)
}

fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
    Rect { x, y, w, h }
}

/// The height of the text box of a style.
fn line_h(style: TextStyle) -> f64 {
    match style {
        TextStyle::Heading => 26.0,
        TextStyle::Strong => 18.0,
        _ => 14.0,
    }
}

/// The width of a mono caption, estimated from its character count.
#[allow(clippy::cast_precision_loss)] // a caption is a handful of characters
fn caption_w(s: &str) -> f64 {
    s.chars().count() as f64 * ADVANCE
}

/// The nodes and targets built so far.
pub(super) struct Page {
    nodes: Vec<Node>,
    targets: Vec<SetupTarget>,
    lang: Lang,
}

impl Page {
    fn new(lang: Lang) -> Self {
        Self {
            nodes: Vec::new(),
            targets: Vec::new(),
            lang,
        }
    }

    fn finish(mut self) -> (DisplayList, Vec<SetupTarget>) {
        self.targets.push(SetupTarget {
            rect: rect(0.0, 0.0, SETUP_W, DRAG_H),
            action: SetupAction::Drag,
        });
        let list = DisplayList {
            nodes: self.nodes,
            size: (SETUP_W, SETUP_H),
        };
        (list, self.targets)
    }

    pub(super) fn word(&self, id: SetupText) -> &'static str {
        setup_text(id, self.lang)
    }

    fn target(&mut self, r: Rect, action: SetupAction) {
        self.targets.push(SetupTarget { rect: r, action });
    }

    /// A line of text at `(x, centre y, width)`.
    fn text(&mut self, at: (f64, f64, f64), s: &str, look: (TextStyle, Ink), align: Align) {
        let h = line_h(look.0);
        self.nodes.push(Node::Text {
            rect: rect(at.0, at.1 - h / 2.0, at.2, h),
            text: Arc::from(s),
            style: look.0,
            ink: look.1,
            align,
            trim: Trim::End,
        });
    }

    /// A line centred in the window.
    pub(super) fn centered(&mut self, cy: f64, s: &str, look: (TextStyle, Ink)) {
        self.text((MARGIN, cy, SETUP_W - 2.0 * MARGIN), s, look, Align::Center);
    }

    /// The fabric with `pressed` ripples pressed (0 to 3).
    pub(super) fn background(&mut self, pressed: u8) {
        self.nodes.push(Node::Image {
            rect: rect(0.0, 0.0, SETUP_W, SETUP_H),
            art: BG_ART[usize::from(pressed.min(3))],
        });
    }

    /// The version label and, when `closable`, the close mark and its target.
    pub(super) fn chrome(&mut self, own: Version, closable: bool) {
        self.text(
            (18.0, 21.0, 160.0),
            &own.to_string(),
            (TextStyle::Label, Ink::Ink),
            Align::Start,
        );
        if !closable {
            return;
        }
        let (x, y, e) = (SETUP_W - 16.0 - 12.0, 14.0, 12.0);
        for points in [vec![(x, y), (x + e, y + e)], vec![(x + e, y), (x, y + e)]] {
            self.nodes.push(Node::Stroke {
                points,
                width: 1.5,
                ink: Ink::Ink,
                dashed: false,
            });
        }
        self.target(rect(x - 12.0, 2.0, 40.0, 36.0), SetupAction::Close);
    }

    /// The step labels, `1 FILES` and so on: all three, or only step `only`.
    pub(super) fn steps(&mut self, only: Option<u8>) {
        let ids = [SetupText::Step1, SetupText::Step2, SetupText::Step3];
        for (n, (&id, &(x, cy))) in (1u8..).zip(ids.iter().zip(STEP_AT.iter())) {
            if only.is_some_and(|o| o != n) {
                continue;
            }
            let s = format!("{n} {}", self.word(id));
            self.text(
                (x, cy, SETUP_W - x),
                &s,
                (TextStyle::Label, Ink::Ink),
                Align::Start,
            );
        }
    }

    /// The black pill at `y` with its lime caption, and its target.
    pub(super) fn button(&mut self, y: f64, id: SetupText, action: SetupAction) {
        let r = rect(BUTTON_X, y, BUTTON_W, BUTTON_H);
        self.nodes.push(Node::Image {
            rect: r,
            art: BUTTON_ART,
        });
        let label = self.word(id);
        self.text(
            (r.x, y + BUTTON_H / 2.0, r.w),
            label,
            (TextStyle::Label, Ink::Flow),
            Align::Center,
        );
        self.target(r, action);
    }

    /// A caption link with an underline, centred at `cy`.
    pub(super) fn link(&mut self, cy: f64, id: SetupText, action: SetupAction) {
        let label = self.word(id);
        let w = caption_w(label);
        let x = (SETUP_W - w) / 2.0;
        self.text(
            (x, cy, w),
            label,
            (TextStyle::Label, Ink::Ink),
            Align::Start,
        );
        let base = cy + 9.0;
        self.nodes.push(Node::Stroke {
            points: vec![(x, base), (x + w, base)],
            width: 1.0,
            ink: Ink::Ink,
            dashed: false,
        });
        self.target(rect(x - 10.0, cy - 12.0, w + 20.0, 24.0), action);
    }

    /// A checkbox with its caption, the pair centred at `cy`.
    pub(super) fn check(&mut self, cy: f64, on: bool, id: SetupText, action: SetupAction) {
        let label = self.word(id);
        let w = caption_w(label);
        let total = CHECK + CHECK_GAP + w;
        let x = (SETUP_W - total) / 2.0;
        let top = cy - CHECK / 2.0;
        self.nodes.push(Node::Stroke {
            points: vec![
                (x, top),
                (x + CHECK, top),
                (x + CHECK, top + CHECK),
                (x, top + CHECK),
                (x, top),
            ],
            width: 1.5,
            ink: Ink::Ink,
            dashed: false,
        });
        if on {
            self.nodes.push(Node::Fill {
                rect: rect(x + 3.0, top + 3.0, CHECK - 6.0, CHECK - 6.0),
                radius: 0.0,
                ink: Ink::Ink,
            });
        }
        self.text(
            (x + CHECK + CHECK_GAP, cy, w),
            label,
            (TextStyle::Label, Ink::Ink),
            Align::Start,
        );
        self.target(rect(x - 10.0, cy - 14.0, total + 20.0, 28.0), action);
    }
}
