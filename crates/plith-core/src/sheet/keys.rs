//! Keyboard behaviour of the sheet: focus order, choice arrows, activation and the city field.
//!
//! Pure: the window maps Win32 key messages to [`SheetKey`], calls [`on_key`], and does what the
//! [`KeyOutcome`] says. An `Act` is applied exactly like the same action from a click.

use super::field::FieldOutcome;
use super::{FocusId, SheetAction, SheetFacts, SheetTarget, SheetView, sheet_build};
use crate::settings::change::SettingChange;

/// A key the sheet understands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SheetKey {
    /// Tab.
    Tab,
    /// Shift+Tab.
    BackTab,
    /// Left arrow.
    Left,
    /// Right arrow.
    Right,
    /// Up arrow.
    Up,
    /// Down arrow.
    Down,
    /// Home.
    Home,
    /// End.
    End,
    /// Backspace.
    Backspace,
    /// Delete.
    Delete,
    /// Enter.
    Enter,
    /// Space.
    Space,
    /// Escape.
    Escape,
    /// Ctrl+A.
    SelectAll,
    /// Ctrl+C.
    Copy,
    /// Ctrl+V with the clipboard text.
    Paste(String),
    /// A typed character.
    Char(char),
}

/// What the window does after a key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyOutcome {
    /// Draw again.
    Redraw,
    /// Apply the action as if it was clicked.
    Act(SheetAction),
    /// Put the text on the clipboard.
    Copy(String),
    /// Close the window.
    Close,
    /// Nothing happened.
    Nothing,
}

fn city_act(text: String) -> KeyOutcome {
    KeyOutcome::Act(SheetAction::Change(SettingChange::City(text)))
}

fn is_field(targets: &[SheetTarget], focus: Option<FocusId>) -> bool {
    focus.is_some_and(|f| {
        targets
            .iter()
            .any(|t| t.id == f && t.action == SheetAction::FocusField)
    })
}

/// Loads the city field with the saved city, caret at the end. Call it when the field gets focus
/// from a click (`SheetAction::FocusField`); the keys do it themselves.
pub fn focus_field(facts: &SheetFacts, view: &mut SheetView) {
    view.field.load(&facts.settings.city);
    view.caret_on = true;
}

/// Commits the field and leaves it (focus cleared; the caller may set a new focus after). The text
/// is returned only when it differs from the saved city, for the caller to apply as
/// `SettingChange::City`. Does nothing when nothing has focus.
///
/// Invariant: whenever the field is not being edited, its text equals `saved` (`focus_field`,
/// Escape and every commit set both), so calling this with another control focused returns `None`.
pub fn commit_field(view: &mut SheetView) -> Option<String> {
    let text = take_commit(view);
    view.focus = None;
    text
}

/// The text to commit if it is news; it becomes the saved value, so a second call gives `None`.
fn take_commit(view: &mut SheetView) -> Option<String> {
    view.focus?;
    if view.field.text == view.field.saved {
        return None;
    }
    view.field.saved.clone_from(&view.field.text);
    Some(view.field.text.clone())
}

fn step(targets: &[SheetTarget], focus: Option<FocusId>, back: bool) -> Option<usize> {
    let n = targets.len();
    let at = focus.and_then(|f| targets.iter().position(|t| t.id == f));
    match (at, back) {
        (None, false) => (n > 0).then_some(0),
        (None, true) => n.checked_sub(1),
        (Some(i), false) => Some((i + 1) % n),
        (Some(i), true) => Some((i + n - 1) % n),
    }
}

fn walk(facts: &SheetFacts, view: &mut SheetView, targets: &[SheetTarget], back: bool) -> bool {
    let Some(i) = step(targets, view.focus, back) else {
        return false;
    };
    view.focus = Some(targets[i].id);
    if targets[i].action == SheetAction::FocusField {
        focus_field(facts, view);
    }
    true
}

/// The neighbour of the focused choice word, when the focus sits in a group of changes.
fn neighbour(targets: &[SheetTarget], focus: Option<FocusId>, back: bool) -> Option<&SheetTarget> {
    let FocusId::Row(row, _) = focus? else {
        return None;
    };
    let group: Vec<&SheetTarget> = targets
        .iter()
        .filter(|t| matches!(t.id, FocusId::Row(r, _) if r == row))
        .filter(|t| matches!(t.action, SheetAction::Change(_)))
        .collect();
    let at = group.iter().position(|t| Some(t.id) == focus)?;
    let to = if back { at.checked_sub(1)? } else { at + 1 };
    group.get(to).copied()
}

fn in_field(
    facts: &SheetFacts,
    view: &mut SheetView,
    targets: &[SheetTarget],
    key: SheetKey,
) -> KeyOutcome {
    match key {
        SheetKey::Tab | SheetKey::BackTab => {
            let committed = take_commit(view);
            walk(facts, view, targets, key == SheetKey::BackTab);
            committed.map_or(KeyOutcome::Redraw, city_act)
        }
        SheetKey::Escape => {
            view.field.load(&facts.settings.city);
            view.focus = None;
            KeyOutcome::Redraw
        }
        key => {
            view.caret_on = true;
            match view.field.edit(&key) {
                FieldOutcome::Edited | FieldOutcome::Moved => KeyOutcome::Redraw,
                FieldOutcome::Copy(t) => KeyOutcome::Copy(t),
                FieldOutcome::Commit(_) => take_commit(view).map_or(KeyOutcome::Redraw, city_act),
                FieldOutcome::Cancel | FieldOutcome::Ignored => KeyOutcome::Nothing,
            }
        }
    }
}

/// Space or Enter outside the field (the field itself is handled by `in_field`).
fn activate(view: &SheetView, targets: &[SheetTarget]) -> KeyOutcome {
    targets
        .iter()
        .find(|t| Some(t.id) == view.focus)
        .map_or(KeyOutcome::Nothing, |t| KeyOutcome::Act(t.action.clone()))
}

/// Applies one key to the view.
pub fn on_key(facts: &SheetFacts, view: &mut SheetView, key: SheetKey) -> KeyOutcome {
    let targets = sheet_build(facts, view).1;
    if is_field(&targets, view.focus) {
        return in_field(facts, view, &targets, key);
    }
    match key {
        SheetKey::Escape => KeyOutcome::Close,
        SheetKey::Tab | SheetKey::BackTab => {
            if walk(facts, view, &targets, key == SheetKey::BackTab) {
                KeyOutcome::Redraw
            } else {
                KeyOutcome::Nothing
            }
        }
        SheetKey::Left | SheetKey::Right | SheetKey::Up | SheetKey::Down => {
            let back = matches!(key, SheetKey::Left | SheetKey::Up);
            let Some(next) = neighbour(&targets, view.focus, back) else {
                return KeyOutcome::Nothing;
            };
            view.focus = Some(next.id);
            KeyOutcome::Act(next.action.clone())
        }
        SheetKey::Space | SheetKey::Enter => activate(view, &targets),
        _ => KeyOutcome::Nothing,
    }
}

#[cfg(test)]
mod tests;
