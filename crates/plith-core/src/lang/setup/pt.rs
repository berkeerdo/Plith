//! Portuguese (Brazilian) installer strings.

use super::SetupText;

/// The Portuguese (Brazilian) string for `t`.
pub fn text(t: SetupText) -> &'static str {
    match t {
        SetupText::Step1 => "ARQUIVOS",
        SetupText::Step2 => "AJUDANTE",
        SetupText::Step3 => "INÍCIO",
        SetupText::LeadStart => "O Plith será instalado neste computador.",
        SetupText::Install => "INSTALAR",
        SetupText::LeadRunning => "Instalando.",
        SetupText::SubUac => "Agora o Windows vai pedir permissão para o ajudante.",
        SetupText::DoneTitle => "O Plith foi instalado.",
        SetupText::DoneSub => "A ilha está esperando no alto da tela.",
        SetupText::Open => "ABRIR O PLITH",
        SetupText::Autostart => "INICIAR COM O WINDOWS",
        SetupText::Close => "FECHAR",
        SetupText::Update => "ATUALIZAR",
        SetupText::UpdateLead => "Uma nova versão do Plith será instalada.",
        SetupText::Retry => "TENTAR DE NOVO",
        SetupText::ErrorTitle => "Não foi possível concluir a instalação.",
        SetupText::NewerTitle => "Este computador já tem um Plith mais recente.",
        SetupText::UninstallLead => "O Plith será removido deste computador.",
        SetupText::Uninstall => "DESINSTALAR",
        SetupText::DeleteData => "APAGAR TAMBÉM MEUS DADOS",
        SetupText::UninstallDone => "O Plith foi removido.",
        SetupText::UacCancelled => "A permissão do Windows foi negada. A instalação não começou.",
        SetupText::ErrorCode => "Código de erro",
        SetupText::Busy => "Outra instalação já está em andamento.",
    }
}
