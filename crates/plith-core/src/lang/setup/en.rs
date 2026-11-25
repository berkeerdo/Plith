//! English installer strings.

use super::SetupText;

/// The English string for `t`.
pub fn text(t: SetupText) -> &'static str {
    match t {
        SetupText::Step1 => "FILES",
        SetupText::Step2 => "HELPER",
        SetupText::Step3 => "START",
        SetupText::LeadStart => "Plith will be installed on this computer.",
        SetupText::Install => "INSTALL",
        SetupText::LeadRunning => "Installing.",
        SetupText::SubUac => "Windows will ask permission for the helper next.",
        SetupText::DoneTitle => "Plith is installed.",
        SetupText::DoneSub => "The island is waiting at the top of your screen.",
        SetupText::Open => "OPEN PLITH",
        SetupText::Autostart => "START WITH WINDOWS",
        SetupText::Close => "CLOSE",
        SetupText::Update => "UPDATE",
        SetupText::UpdateLead => "A new version of Plith will be installed.",
        SetupText::Retry => "TRY AGAIN",
        SetupText::ErrorTitle => "Setup could not finish.",
        SetupText::NewerTitle => "A newer Plith is already on this computer.",
        SetupText::UninstallLead => "Plith will be removed from this computer.",
        SetupText::Uninstall => "UNINSTALL",
        SetupText::DeleteData => "ALSO DELETE MY DATA",
        SetupText::UninstallDone => "Plith is removed.",
        SetupText::UacCancelled => "Windows permission was declined. Setup has not started.",
        SetupText::ErrorCode => "Error code",
        SetupText::Busy => "Another setup is already running.",
    }
}
