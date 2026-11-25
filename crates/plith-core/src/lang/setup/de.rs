//! German installer strings.

use super::SetupText;

/// The German string for `t`.
pub fn text(t: SetupText) -> &'static str {
    match t {
        SetupText::Step1 => "DATEIEN",
        SetupText::Step2 => "HELFER",
        SetupText::Step3 => "START",
        SetupText::LeadStart => "Plith wird auf diesem Computer installiert.",
        SetupText::Install => "INSTALLIEREN",
        SetupText::LeadRunning => "Installation läuft.",
        SetupText::SubUac => "Windows fragt gleich nach der Erlaubnis für den Helfer.",
        SetupText::DoneTitle => "Plith ist installiert.",
        SetupText::DoneSub => "Die Insel wartet am oberen Bildschirmrand.",
        SetupText::Open => "PLITH ÖFFNEN",
        SetupText::Autostart => "MIT WINDOWS STARTEN",
        SetupText::Close => "SCHLIESSEN",
        SetupText::Update => "AKTUALISIEREN",
        SetupText::UpdateLead => "Eine neue Version von Plith wird installiert.",
        SetupText::Retry => "NOCHMAL",
        SetupText::ErrorTitle => "Die Installation konnte nicht abgeschlossen werden.",
        SetupText::NewerTitle => "Auf diesem Computer ist schon ein neueres Plith.",
        SetupText::UninstallLead => "Plith wird von diesem Computer entfernt.",
        SetupText::Uninstall => "ENTFERNEN",
        SetupText::DeleteData => "AUCH MEINE DATEN LÖSCHEN",
        SetupText::UninstallDone => "Plith wurde entfernt.",
        SetupText::UacCancelled => {
            "Die Windows-Erlaubnis wurde abgelehnt. Die Installation hat nicht begonnen."
        }
    }
}
