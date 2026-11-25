//! Polish installer strings.

use super::SetupText;

/// The Polish string for `t`.
pub fn text(t: SetupText) -> &'static str {
    match t {
        SetupText::Step1 => "PLIKI",
        SetupText::Step2 => "POMOCNIK",
        SetupText::Step3 => "START",
        SetupText::LeadStart => "Plith zostanie zainstalowany na tym komputerze.",
        SetupText::Install => "ZAINSTALUJ",
        SetupText::LeadRunning => "Trwa instalacja.",
        SetupText::SubUac => "Windows zaraz poprosi o zgodę dla pomocnika.",
        SetupText::DoneTitle => "Plith jest zainstalowany.",
        SetupText::DoneSub => "Wyspa czeka na górze ekranu.",
        SetupText::Open => "OTWÓRZ PLITH",
        SetupText::Autostart => "URUCHAMIAJ Z WINDOWS",
        SetupText::Close => "ZAMKNIJ",
        SetupText::Update => "ZAKTUALIZUJ",
        SetupText::UpdateLead => "Zostanie zainstalowana nowa wersja Plith.",
        SetupText::Retry => "SPRÓBUJ PONOWNIE",
        SetupText::ErrorTitle => "Nie udało się dokończyć instalacji.",
        SetupText::NewerTitle => "Na tym komputerze jest już nowszy Plith.",
        SetupText::UninstallLead => "Plith zostanie usunięty z tego komputera.",
        SetupText::Uninstall => "ODINSTALUJ",
        SetupText::DeleteData => "USUŃ TAKŻE MOJE DANE",
        SetupText::UninstallDone => "Plith został usunięty.",
        SetupText::UacCancelled => "Zgoda Windows została odrzucona. Instalacja nie ruszyła.",
    }
}
