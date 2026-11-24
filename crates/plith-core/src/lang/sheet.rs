//! Settings window copy in the eight languages. See ADR 0015.

use super::Lang;
use crate::theme::Theme;

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

/// A string of the settings window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SheetText {
    /// Title of the settings window.
    WindowTitle,
    /// Caption of the settings sheet.
    Caption,
    /// Sidebar name of the General section.
    SectionGeneral,
    /// Sidebar name of the Look section.
    SectionLook,
    /// Sidebar name of the Idle section.
    SectionIdle,
    /// Sidebar name of the OSD section.
    SectionOsd,
    /// Sidebar name of the About section.
    SectionAbout,
    /// Line under the General section title.
    LineGeneral,
    /// Line under the Look section title.
    LineLook,
    /// Line under the Idle section title.
    LineIdle,
    /// Line under the OSD section title.
    LineOsd,
    /// Line under the About section title.
    LineAbout,
    /// Label of the language row.
    RowLanguage,
    /// Sub line of the language row.
    SubLanguage,
    /// Label of the row choosing the first page on open.
    RowOpens,
    /// Sub line of the open row.
    SubOpens,
    /// Label of the colour theme row.
    RowTheme,
    /// Sub line of the colour theme row.
    SubTheme,
    /// Caption above the closed island preview.
    CaptionClosed,
    /// Note under the closed island preview.
    PreviewNote,
    /// Label of the strip row.
    RowStrip,
    /// Sub line of the strip row.
    SubStrip,
    /// Label of the top right row.
    RowTopRight,
    /// Sub line of the top right row.
    SubTopRight,
    /// Label of the played today row.
    RowPlayed,
    /// Sub line of the played today row.
    SubPlayed,
    /// Label of the weather row.
    RowWeather,
    /// Sub line of the weather row.
    SubWeather,
    /// Label of the location row.
    RowLocation,
    /// Sub line of the location row.
    SubLocation,
    /// Label of the temperature unit row.
    RowUnit,
    /// Label of the on-screen display row.
    RowOsd,
    /// Sub line of the on-screen display row.
    SubOsd,
    /// Label of the hide Windows indicator row.
    RowHideWindows,
    /// Sub line of the hide Windows indicator row.
    SubHideWindows,
    /// Label of the version row.
    RowVersion,
    /// Label of the settings file row.
    RowFile,
    /// Label of the data row.
    RowData,
    /// Sub line of the data row.
    SubData,
    /// Choice: automatic.
    WordAuto,
    /// Choice: the welcome view.
    WordWelcome,
    /// Choice: the last page.
    WordLastPage,
    /// Choice: plus or minus three hours.
    WordHours3,
    /// Choice: plus or minus six hours.
    WordHours6,
    /// Choice: the whole day.
    WordAllDay,
    /// Choice: weather.
    WordWeather,
    /// Choice: sound.
    WordSound,
    /// Choice: what plays.
    WordPlaying,
    /// Choice: nothing.
    WordEmpty,
    /// Choice: the Windows location.
    WordWindows,
    /// Choice: the IP location.
    WordIp,
    /// Choice: a city.
    WordCity,
    /// Choice: degrees Celsius.
    WordCelsius,
    /// Choice: degrees Fahrenheit.
    WordFahrenheit,
    /// Switch state: on.
    WordOn,
    /// Switch state: off.
    WordOff,
    /// Button: open.
    WordOpen,
    /// Button: all settings.
    WordAllSettings,
    /// Shown when the settings file cannot be written.
    SaveError,
}

impl SheetText {
    /// Every variant, for exhaustive checks.
    pub const ALL: [SheetText; 59] = [
        SheetText::WindowTitle,
        SheetText::Caption,
        SheetText::SectionGeneral,
        SheetText::SectionLook,
        SheetText::SectionIdle,
        SheetText::SectionOsd,
        SheetText::SectionAbout,
        SheetText::LineGeneral,
        SheetText::LineLook,
        SheetText::LineIdle,
        SheetText::LineOsd,
        SheetText::LineAbout,
        SheetText::RowLanguage,
        SheetText::SubLanguage,
        SheetText::RowOpens,
        SheetText::SubOpens,
        SheetText::RowTheme,
        SheetText::SubTheme,
        SheetText::CaptionClosed,
        SheetText::PreviewNote,
        SheetText::RowStrip,
        SheetText::SubStrip,
        SheetText::RowTopRight,
        SheetText::SubTopRight,
        SheetText::RowPlayed,
        SheetText::SubPlayed,
        SheetText::RowWeather,
        SheetText::SubWeather,
        SheetText::RowLocation,
        SheetText::SubLocation,
        SheetText::RowUnit,
        SheetText::RowOsd,
        SheetText::SubOsd,
        SheetText::RowHideWindows,
        SheetText::SubHideWindows,
        SheetText::RowVersion,
        SheetText::RowFile,
        SheetText::RowData,
        SheetText::SubData,
        SheetText::WordAuto,
        SheetText::WordWelcome,
        SheetText::WordLastPage,
        SheetText::WordHours3,
        SheetText::WordHours6,
        SheetText::WordAllDay,
        SheetText::WordWeather,
        SheetText::WordSound,
        SheetText::WordPlaying,
        SheetText::WordEmpty,
        SheetText::WordWindows,
        SheetText::WordIp,
        SheetText::WordCity,
        SheetText::WordCelsius,
        SheetText::WordFahrenheit,
        SheetText::WordOn,
        SheetText::WordOff,
        SheetText::WordOpen,
        SheetText::WordAllSettings,
        SheetText::SaveError,
    ];

    /// Whether the string is drawn in the upper-cased caption style (`Label`), so its copy is
    /// upper case under its language's rules.
    #[must_use]
    pub fn is_caption(self) -> bool {
        matches!(
            self,
            SheetText::Caption
                | SheetText::CaptionClosed
                | SheetText::WordAuto
                | SheetText::WordWelcome
                | SheetText::WordLastPage
                | SheetText::WordHours3
                | SheetText::WordHours6
                | SheetText::WordAllDay
                | SheetText::WordWeather
                | SheetText::WordSound
                | SheetText::WordPlaying
                | SheetText::WordEmpty
                | SheetText::WordWindows
                | SheetText::WordIp
                | SheetText::WordCity
                | SheetText::WordCelsius
                | SheetText::WordFahrenheit
                | SheetText::WordOn
                | SheetText::WordOff
                | SheetText::WordOpen
                | SheetText::WordAllSettings
        )
    }
}

/// The string for `t` in `lang`.
#[must_use]
pub fn sheet_text(t: SheetText, lang: Lang) -> &'static str {
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

/// A theme named by its product name, upper case under Turkish rules. The names are proper
/// names and read the same in every language (ADR 0007, theme switch spec section 3).
#[must_use]
pub fn theme_name(theme: Theme) -> &'static str {
    match theme {
        Theme::Gece => "GECE",
        Theme::Kagit => "KA\u{11e}IT",
        Theme::York => "YORK",
        Theme::Sodyum => "SODYUM",
    }
}

/// A language named in itself, for the language picker.
#[must_use]
pub fn lang_name(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "English",
        Lang::Tr => "T\u{fc}rk\u{e7}e",
        Lang::De => "Deutsch",
        Lang::Es => "Espa\u{f1}ol",
        Lang::Fr => "Fran\u{e7}ais",
        Lang::Pt => "Portugu\u{ea}s",
        Lang::It => "Italiano",
        Lang::Pl => "Polski",
    }
}
