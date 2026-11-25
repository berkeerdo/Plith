//! French installer strings.

use super::SetupText;

/// The French string for `t`.
pub fn text(t: SetupText) -> &'static str {
    match t {
        SetupText::Step1 => "FICHIERS",
        SetupText::Step2 => "ASSISTANT",
        SetupText::Step3 => "LANCEMENT",
        SetupText::LeadStart => "Plith va être installé sur cet ordinateur.",
        SetupText::Install => "INSTALLER",
        SetupText::LeadRunning => "Installation en cours.",
        SetupText::SubUac => "Windows va maintenant demander l’autorisation pour l’assistant.",
        SetupText::DoneTitle => "Plith est installé.",
        SetupText::DoneSub => "L’île vous attend en haut de l’écran.",
        SetupText::Open => "OUVRIR PLITH",
        SetupText::Autostart => "LANCER AVEC WINDOWS",
        SetupText::Close => "FERMER",
        SetupText::Update => "METTRE À JOUR",
        SetupText::UpdateLead => "Une nouvelle version de Plith va être installée.",
        SetupText::Retry => "RÉESSAYER",
        SetupText::ErrorTitle => "L’installation n’a pas pu se terminer.",
        SetupText::NewerTitle => "Une version plus récente de Plith est déjà installée.",
        SetupText::UninstallLead => "Plith va être supprimé de cet ordinateur.",
        SetupText::Uninstall => "DÉSINSTALLER",
        SetupText::DeleteData => "SUPPRIMER AUSSI MES DONNÉES",
        SetupText::UninstallDone => "Plith a été supprimé.",
        SetupText::UacCancelled => {
            "L’autorisation Windows a été refusée. L’installation n’a pas commencé."
        }
        SetupText::ErrorCode => "Code d’erreur",
        SetupText::Busy => "Une autre installation est déjà en cours.",
    }
}
