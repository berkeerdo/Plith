//! Italian installer strings.

use super::SetupText;

/// The Italian string for `t`.
pub fn text(t: SetupText) -> &'static str {
    match t {
        SetupText::Step1 => "FILE",
        SetupText::Step2 => "ASSISTENTE",
        SetupText::Step3 => "AVVIO",
        SetupText::LeadStart => "Plith verrà installato su questo computer.",
        SetupText::Install => "INSTALLA",
        SetupText::LeadRunning => "Installazione in corso.",
        SetupText::SubUac => "Windows chiederà ora il permesso per l’assistente.",
        SetupText::DoneTitle => "Plith è installato.",
        SetupText::DoneSub => "L’isola ti aspetta in cima allo schermo.",
        SetupText::Open => "APRI PLITH",
        SetupText::Autostart => "AVVIA CON WINDOWS",
        SetupText::Close => "CHIUDI",
        SetupText::Update => "AGGIORNA",
        SetupText::UpdateLead => "Verrà installata una nuova versione di Plith.",
        SetupText::Retry => "RIPROVA",
        SetupText::ErrorTitle => "Non è stato possibile completare l’installazione.",
        SetupText::NewerTitle => "Su questo computer c’è già un Plith più recente.",
        SetupText::UninstallLead => "Plith verrà rimosso da questo computer.",
        SetupText::Uninstall => "DISINSTALLA",
        SetupText::DeleteData => "ELIMINA ANCHE I MIEI DATI",
        SetupText::UninstallDone => "Plith è stato rimosso.",
        SetupText::UacCancelled => {
            "Il permesso di Windows è stato negato. L’installazione non è iniziata."
        }
    }
}
