//! Turkish installer strings.

use super::SetupText;

/// The Turkish string for `t`.
pub fn text(t: SetupText) -> &'static str {
    match t {
        SetupText::Step1 => "DOSYALAR",
        SetupText::Step2 => "YARDIMCI",
        SetupText::Step3 => "BAŞLAT",
        SetupText::LeadStart => "Plith bu bilgisayara kurulacak.",
        SetupText::Install => "KUR",
        SetupText::LeadRunning => "Kuruluyor.",
        SetupText::SubUac => "Windows şimdi yardımcıya izin isteyecek.",
        SetupText::DoneTitle => "Plith kuruldu.",
        SetupText::DoneSub => "Ada ekranın üstünde seni bekliyor.",
        SetupText::Open => "PLITH’İ AÇ",
        SetupText::Autostart => "WINDOWS İLE BAŞLAT",
        SetupText::Close => "KAPAT",
        SetupText::Update => "GÜNCELLE",
        SetupText::UpdateLead => "Plith’in yeni sürümü kurulacak.",
        SetupText::Retry => "TEKRAR DENE",
        SetupText::ErrorTitle => "Kurulum tamamlanamadı.",
        SetupText::NewerTitle => "Bu bilgisayarda daha yeni bir Plith var.",
        SetupText::UninstallLead => "Plith bu bilgisayardan kaldırılacak.",
        SetupText::Uninstall => "KALDIR",
        SetupText::DeleteData => "KULLANICI VERİLERİNİ DE SİL",
        SetupText::UninstallDone => "Plith kaldırıldı.",
        SetupText::UacCancelled => "Windows izni verilmedi. Kurulum başlamadı.",
        SetupText::ErrorCode => "Hata kodu",
        SetupText::Busy => "Başka bir kurulum sürüyor.",
    }
}
