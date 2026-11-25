//! One edit of the settings and what it makes the running app redo.

use super::{
    FlyoutPref, HeadSlot, IdleLook, LangChoice, LocationMode, OpenOn, RunSpan, Settings, TempUnit,
};
use crate::theme::Theme;

/// One setting the window can change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingChange {
    /// The resting look.
    Look(IdleLook),
    /// The run chart span.
    Span(RunSpan),
    /// The welcome head slot.
    Head(HeadSlot),
    /// The plays chart on or off.
    Plays(bool),
    /// Weather on or off.
    Weather(bool),
    /// Where the weather location comes from.
    Location(LocationMode),
    /// The city, free text.
    City(String),
    /// The temperature unit.
    Unit(TempUnit),
    /// The page the island opens on.
    OpenOn(OpenOn),
    /// The interface language.
    Lang(LangChoice),
    /// The OSD on or off.
    Osd(bool),
    /// What to do with the Windows flyout.
    Flyout(FlyoutPref),
    /// The colour theme.
    Theme(Theme),
    /// The daily update check on or off.
    UpdateCheck(bool),
}

/// What a change makes the running app redo.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)] // independent redo flags, any mix can be set
pub struct Effects {
    /// Rebuild the resting pill.
    pub pill: bool,
    /// Rebuild the welcome view.
    pub welcome: bool,
    /// Restart the weather service.
    pub weather: bool,
    /// Switch the interface language.
    pub lang: bool,
    /// Restart the OSD side.
    pub osd: bool,
    /// Switch the palette; the page bitmaps are redrawn in it.
    pub theme: bool,
    /// Start or stop the update check service.
    pub update: bool,
}

/// Stores `new` in `slot` and returns whether it differed.
fn set<T: PartialEq>(slot: &mut T, new: T) -> bool {
    if *slot == new {
        return false;
    }
    *slot = new;
    true
}

/// Applies `change` and returns the effects it needs; the default (nothing to redo) when the
/// value was already set. A look change reports only the pill: the quick page is the
/// controller's business.
pub fn apply_change(settings: &mut Settings, change: SettingChange) -> Effects {
    let mut effects = Effects::default();
    match change {
        SettingChange::Look(v) => effects.pill = set(&mut settings.look, v),
        SettingChange::Span(v) => effects.welcome = set(&mut settings.span, v),
        SettingChange::Head(v) => effects.welcome = set(&mut settings.head, v),
        SettingChange::Plays(v) => effects.welcome = set(&mut settings.plays, v),
        SettingChange::Unit(v) => {
            let changed = set(&mut settings.unit, v);
            effects.welcome = changed;
            effects.pill = changed;
        }
        SettingChange::OpenOn(v) => effects.welcome = set(&mut settings.open_on, v),
        SettingChange::Weather(v) => effects.weather = set(&mut settings.weather, v),
        SettingChange::Location(v) => effects.weather = set(&mut settings.location, v),
        SettingChange::City(v) => effects.weather = set(&mut settings.city, v),
        SettingChange::Lang(v) => effects.lang = set(&mut settings.lang, v),
        SettingChange::Osd(v) => effects.osd = set(&mut settings.osd, v),
        SettingChange::Flyout(v) => effects.osd = set(&mut settings.flyout, v),
        SettingChange::Theme(v) => effects.theme = set(&mut settings.theme, v),
        SettingChange::UpdateCheck(v) => effects.update = set(&mut settings.update_check, v),
    }
    effects
}

#[cfg(test)]
mod tests;
