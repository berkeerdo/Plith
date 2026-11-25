//! Spanish installer strings.

use super::SetupText;

/// The Spanish string for `t`.
pub fn text(t: SetupText) -> &'static str {
    match t {
        SetupText::Step1 => "ARCHIVOS",
        SetupText::Step2 => "AYUDANTE",
        SetupText::Step3 => "INICIO",
        SetupText::LeadStart => "Plith se instalará en este equipo.",
        SetupText::Install => "INSTALAR",
        SetupText::LeadRunning => "Instalando.",
        SetupText::SubUac => "Windows pedirá ahora permiso para el ayudante.",
        SetupText::DoneTitle => "Plith está instalado.",
        SetupText::DoneSub => "La isla te espera en la parte superior de la pantalla.",
        SetupText::Open => "ABRIR PLITH",
        SetupText::Autostart => "INICIAR CON WINDOWS",
        SetupText::Close => "CERRAR",
        SetupText::Update => "ACTUALIZAR",
        SetupText::UpdateLead => "Se instalará una versión nueva de Plith.",
        SetupText::Retry => "REINTENTAR",
        SetupText::ErrorTitle => "No se pudo terminar la instalación.",
        SetupText::NewerTitle => "Este equipo ya tiene un Plith más reciente.",
        SetupText::UninstallLead => "Plith se quitará de este equipo.",
        SetupText::Uninstall => "DESINSTALAR",
        SetupText::DeleteData => "BORRAR TAMBIÉN MIS DATOS",
        SetupText::UninstallDone => "Plith se desinstaló.",
        SetupText::UacCancelled => "Se rechazó el permiso de Windows. La instalación no empezó.",
    }
}
