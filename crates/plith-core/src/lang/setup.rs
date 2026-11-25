//! Installer copy in the eight languages (R7 part 2). The screens are drawn from these strings;
//! the version is drawn from `Version`'s own display, so it has no entry here.

use super::Lang;

mod de;
mod en;
mod es;
#[doc(hidden)]
pub mod fit;
mod fr;
mod it;
mod pl;
mod pt;
mod tr;

#[cfg(test)]
mod tests;

/// A string of the installer, updater and uninstaller screens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupText {
    /// Label of the first step: the files.
    Step1,
    /// Label of the second step: the elevated helper.
    Step2,
    /// Label of the third step: starting Plith.
    Step3,
    /// Lead line of the start screen.
    LeadStart,
    /// Label of the install button.
    Install,
    /// Lead line while installing.
    LeadRunning,
    /// Sub line naming the Windows prompt that comes next.
    SubUac,
    /// Title of the done screen.
    DoneTitle,
    /// Sub line of the done screen.
    DoneSub,
    /// Label of the button that opens Plith.
    Open,
    /// Label of the start with Windows check.
    Autostart,
    /// Label of the close link.
    Close,
    /// Label of the update button.
    Update,
    /// Lead line of the update screen.
    UpdateLead,
    /// Label of the retry button.
    Retry,
    /// Title of the failure screen.
    ErrorTitle,
    /// Title shown when a newer Plith is already installed.
    NewerTitle,
    /// Lead line of the uninstall screen.
    UninstallLead,
    /// Label of the uninstall button.
    Uninstall,
    /// Label of the check that also deletes the user's data.
    DeleteData,
    /// Title of the screen after uninstalling.
    UninstallDone,
    /// One line on the start screen after the user declined the Windows prompt.
    UacCancelled,
}

impl SetupText {
    /// Every installer string.
    pub const ALL: [SetupText; 22] = [
        SetupText::Step1,
        SetupText::Step2,
        SetupText::Step3,
        SetupText::LeadStart,
        SetupText::Install,
        SetupText::LeadRunning,
        SetupText::SubUac,
        SetupText::DoneTitle,
        SetupText::DoneSub,
        SetupText::Open,
        SetupText::Autostart,
        SetupText::Close,
        SetupText::Update,
        SetupText::UpdateLead,
        SetupText::Retry,
        SetupText::ErrorTitle,
        SetupText::NewerTitle,
        SetupText::UninstallLead,
        SetupText::Uninstall,
        SetupText::DeleteData,
        SetupText::UninstallDone,
        SetupText::UacCancelled,
    ];

    /// Whether the string is a caption: written upper case in its language and set in `Label`.
    #[must_use]
    pub fn is_caption(self) -> bool {
        matches!(
            self,
            SetupText::Step1
                | SetupText::Step2
                | SetupText::Step3
                | SetupText::Install
                | SetupText::Open
                | SetupText::Autostart
                | SetupText::Close
                | SetupText::Update
                | SetupText::Retry
                | SetupText::Uninstall
                | SetupText::DeleteData
        )
    }
}

/// The string for `t` in `lang`.
#[must_use]
pub fn setup_text(t: SetupText, lang: Lang) -> &'static str {
    match lang {
        Lang::En => en::text(t),
        Lang::Tr => tr::text(t),
        Lang::De => de::text(t),
        Lang::Es => es::text(t),
        Lang::Fr => fr::text(t),
        Lang::Pt => pt::text(t),
        Lang::It => it::text(t),
        Lang::Pl => pl::text(t),
    }
}
