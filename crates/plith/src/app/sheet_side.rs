//! The settings window side of the app: the window, its render thread and view state, and the
//! routing of window events to the steps the app takes. Routing is pure ([`route`]); the app
//! owns the settings and applies every change.

use plith_core::draw::{DisplayList, TextStyle};
use plith_core::settings::change::SettingChange;
use plith_core::sheet::{
    KeyOutcome, SheetAction, SheetFacts, SheetTarget, SheetView, commit_field, focus_field, on_key,
    sheet_build, sheet_hit,
};
use plith_core::theme::Palette;
use plith_render::TextMeasure;
use plith_render::sheet::{SheetConfig, SheetRender};
use plith_shell::sheet_window::{SheetEvent, SheetWindow};
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// What the app does after a window event.
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    /// Draw again: the view changed.
    Redraw,
    /// Apply and save a setting.
    Change(SettingChange),
    /// Close the window.
    Close,
    /// Put the text on the clipboard.
    Copy(String),
    /// Open the settings folder.
    OpenFolder,
    /// The client area is now this many pixels at this scale.
    Resize(u32, u32, f64),
}

/// An empty list, for previews before the idle model exists.
#[must_use]
pub fn blank() -> DisplayList {
    DisplayList {
        nodes: Vec::new(),
        size: (0.0, 0.0),
    }
}

/// Whether keyboard focus is on the city field.
#[must_use]
pub fn field_focused(view: &SheetView, targets: &[SheetTarget]) -> bool {
    view.focus.is_some_and(|focus| {
        targets
            .iter()
            .any(|t| t.id == focus && t.action == SheetAction::FocusField)
    })
}

/// Turns one window event into the steps the app takes, updating `view`. `targets` are the
/// click targets of the sheet as last built from `facts` and `view`.
pub fn route(
    event: SheetEvent,
    facts: &SheetFacts,
    view: &mut SheetView,
    targets: &[SheetTarget],
) -> Vec<Step> {
    match event {
        SheetEvent::Move(x, y) => hover(view, sheet_hit(targets, x, y).map(|t| t.id)),
        SheetEvent::Leave => hover(view, None),
        SheetEvent::Click(x, y) => click(facts, view, sheet_hit(targets, x, y)),
        SheetEvent::Key(key) => match on_key(facts, view, key) {
            KeyOutcome::Redraw => vec![Step::Redraw],
            KeyOutcome::Act(action) => {
                let mut steps = Vec::new();
                act(facts, view, action, &mut steps);
                steps
            }
            KeyOutcome::Copy(text) => vec![Step::Copy(text)],
            KeyOutcome::Close => close(view),
            KeyOutcome::Nothing => Vec::new(),
        },
        SheetEvent::Resized {
            width_px,
            height_px,
            scale,
        } => vec![Step::Resize(width_px, height_px, scale)],
        SheetEvent::Closed => close(view),
    }
}

fn hover(view: &mut SheetView, id: Option<plith_core::sheet::FocusId>) -> Vec<Step> {
    if view.hover == id {
        return Vec::new();
    }
    view.hover = id;
    vec![Step::Redraw]
}

/// The city typed so far, as a change, when it differs from the saved one; the field loses
/// focus either way.
fn commit(view: &mut SheetView) -> Vec<Step> {
    commit_field(view)
        .map(|city| Step::Change(SettingChange::City(city)))
        .into_iter()
        .collect()
}

fn click(facts: &SheetFacts, view: &mut SheetView, hit: Option<&SheetTarget>) -> Vec<Step> {
    if let Some(field) = hit.filter(|t| t.action == SheetAction::FocusField) {
        if view.focus == Some(field.id) {
            // Already editing: keep the text and the caret.
            return Vec::new();
        }
        let mut steps = commit(view);
        view.focus = Some(field.id);
        focus_field(facts, view);
        steps.push(Step::Redraw);
        return steps;
    }
    let had_focus = view.focus.is_some();
    let mut steps = commit(view);
    match hit {
        Some(target) => act(facts, view, target.action.clone(), &mut steps),
        None if had_focus => steps.push(Step::Redraw),
        None => {}
    }
    steps
}

/// A click's or a key's action.
fn act(facts: &SheetFacts, view: &mut SheetView, action: SheetAction, steps: &mut Vec<Step>) {
    match action {
        SheetAction::Section(section) => {
            view.section = section;
            steps.push(Step::Redraw);
        }
        SheetAction::Change(change) => steps.push(Step::Change(change)),
        SheetAction::FocusField => {
            focus_field(facts, view);
            steps.push(Step::Redraw);
        }
        SheetAction::OpenFolder => steps.push(Step::OpenFolder),
    }
}

/// Closing commits a city still being typed, as losing focus does.
fn close(view: &mut SheetView) -> Vec<Step> {
    let mut steps = commit(view);
    steps.push(Step::Close);
    steps
}

/// The open settings window, its render thread and what it shows.
pub struct SheetSide {
    window: SheetWindow,
    render: Option<SheetRender>,
    /// Measures the field text; `None` when DirectWrite failed (the caret then stays at 0).
    measure: Option<TextMeasure>,
    /// The view state.
    pub view: SheetView,
    /// The targets of the last build.
    targets: Vec<SheetTarget>,
    /// True when the view or the facts changed since the last build.
    stale: bool,
    /// The last list built and not yet handed to the render thread.
    pending: Option<DisplayList>,
    /// The field text and caret the caret position was measured for.
    measured: Option<(String, usize)>,
    /// The title's text.
    title: String,
    /// Set by the window's panic report.
    panicked: Rc<Cell<bool>>,
    /// Set by the render thread when it ends on an error.
    dead: Arc<AtomicBool>,
}

/// What opening the window needs besides the facts.
pub struct Opening {
    /// The monitor to center on (`HMONITOR` value).
    pub monitor: isize,
    /// The window title.
    pub title: String,
    /// The window's events, called on the UI thread.
    pub on_event: Box<dyn FnMut(SheetEvent)>,
    /// Called with the message when the window's callback panics.
    pub on_panic: Box<dyn Fn(String)>,
    /// Called from the render thread with the message when it ends on an error.
    pub on_fatal: Box<dyn Fn(String) + Send>,
}

impl SheetSide {
    /// Opens the window on `view` and starts its render thread.
    ///
    /// # Errors
    /// Returns a log line when the window or its render thread cannot start.
    pub fn open(opening: Opening, view: SheetView) -> Result<SheetSide, String> {
        let panicked = Rc::new(Cell::new(false));
        let on_panic = {
            let panicked = Rc::clone(&panicked);
            let report = opening.on_panic;
            Box::new(move |message: String| {
                panicked.set(true);
                report(message);
            })
        };
        let window = SheetWindow::open(opening.monitor, &opening.title, opening.on_event, on_panic)
            .map_err(|error| format!("settings window: cannot open: {error}"))?;
        let dead = Arc::new(AtomicBool::new(false));
        let on_fatal = {
            let dead = Arc::clone(&dead);
            let report = opening.on_fatal;
            Box::new(move |message: String| {
                dead.store(true, Ordering::Release);
                report(message);
            })
        };
        let (width_px, height_px, scale) = window.client_px();
        let config = SheetConfig {
            hwnd: window.hwnd(),
            width_px,
            height_px,
            scale,
            palette: Palette::default(),
        };
        let render = SheetRender::spawn(config, on_fatal)
            .map_err(|error| format!("settings window: render failed to start: {error}"))?;
        Ok(SheetSide {
            window,
            render: Some(render),
            measure: TextMeasure::new().ok(),
            view,
            targets: Vec::new(),
            stale: true,
            pending: None,
            measured: None,
            title: opening.title,
            panicked,
            dead,
        })
    }

    /// Restores and focuses the open window.
    pub fn raise(&self) {
        self.window.raise();
    }

    /// Whether the window callback panicked or the render thread died: the app closes it.
    #[must_use]
    pub fn broken(&self) -> bool {
        self.panicked.get() || self.dead.load(Ordering::Acquire)
    }

    /// Sets the caption when it differs.
    pub fn set_title(&mut self, title: &str) {
        if self.title != title {
            title.clone_into(&mut self.title);
            self.window.set_title(title);
        }
    }

    /// Marks the list for a rebuild: the facts changed.
    pub fn touch(&mut self) {
        self.stale = true;
    }

    /// Routes one event against targets built from `facts`.
    pub fn route(&mut self, event: SheetEvent, facts: &SheetFacts) -> Vec<Step> {
        self.build(facts);
        let steps = route(event, facts, &mut self.view, &self.targets);
        if !steps.is_empty() {
            self.stale = true;
        }
        steps
    }

    /// Hands the client size to the render thread.
    pub fn resize(&self, width_px: u32, height_px: u32, scale: f64) {
        if let Some(render) = &self.render {
            render.resize(width_px, height_px, scale);
        }
    }

    /// Whether the caret blinks now.
    #[must_use]
    pub fn caret_wanted(&self) -> bool {
        field_focused(&self.view, &self.targets)
    }

    /// Flips the caret.
    pub fn blink(&mut self) {
        self.view.caret_on = !self.view.caret_on;
        self.stale = true;
    }

    /// Rebuilds the list and the targets when stale and publishes a list not yet published.
    pub fn publish(&mut self, facts: &SheetFacts) {
        self.build(facts);
        if let (Some(list), Some(render)) = (self.pending.take(), &self.render) {
            render.publish(Arc::new(list));
        }
    }

    /// Rebuilds the list and the targets when stale; the list waits in `pending`.
    fn build(&mut self, facts: &SheetFacts) {
        if !self.stale {
            return;
        }
        self.measure_field();
        let (list, targets) = sheet_build(facts, &self.view);
        self.targets = targets;
        self.pending = Some(list);
        self.stale = false;
    }

    /// Fills the caret position and the text width for the field's text, when either moved.
    fn measure_field(&mut self) {
        let field = &self.view.field;
        let now = (field.text.clone(), field.caret);
        if self.measured.as_ref() == Some(&now) {
            return;
        }
        let Some(measure) = self.measure.as_mut() else {
            return;
        };
        let before: String = field.text.chars().take(field.caret).collect();
        self.view.caret_x = measure.width(&before, TextStyle::Body);
        self.view.text_w = measure.width(&field.text, TextStyle::Body);
        self.measured = Some(now);
    }

    /// Stops the render thread, then destroys the window; the view goes back to the caller.
    #[must_use]
    pub fn close(mut self) -> SheetView {
        if let Some(render) = self.render.take() {
            render.stop_and_join();
        }
        let view = std::mem::take(&mut self.view);
        self.window.close();
        view
    }
}

#[cfg(test)]
mod tests;
