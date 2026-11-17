//! Our own Turkish label copy, written upper-cased with Turkish rules, and the helper that
//! upper-cases it. Names that come from Windows or Voicemeeter (devices, endpoints, strips, apps,
//! track titles) never go through [`tr_upper`]: the renderer upper-cases them invariantly.

/// Tab label of the audio page.
pub const TAB_AUDIO: &str = "SES";
/// Tab label of the media page.
pub const TAB_MEDIA: &str = "MEDYA";
/// Tab label of the shelf page.
pub const TAB_SHELF: &str = "RAF";
/// Tab label of the system page.
pub const TAB_SYSTEM: &str = "SİSTEM";
/// A muted source or output.
pub const MUTED_LABEL: &str = "SESSİZ";
/// A muted microphone or output in the pill.
pub const OFF_LABEL: &str = "KAPALI";
/// Brightness without DDC/CI.
pub const DDC_OFF: &str = "DESTEKLENMİYOR · MONİTÖRDE DDC/CI KAPALI";
/// The empty shelf's caption.
pub const SHELF_HINT: &str = "RAFTA BEKLER, İSTEDİĞİN YERE GERİ SÜRÜKLERSİN";
/// The first word of the default output's caption (`VARSAYILAN · KULAKLIK`).
pub const DEFAULT_CAPTION: &str = "VARSAYILAN";
/// The label above the other outputs.
pub const OTHER_OUTPUTS: &str = "DİĞER ÇIKIŞLAR";
/// Shown when there is no output device at all.
pub const NO_OUTPUT: &str = "ÇIKIŞ YOK";
/// Kind caption of headphones and headsets.
pub const KIND_HEADPHONES: &str = "KULAKLIK";
/// Kind caption of loudspeakers.
pub const KIND_SPEAKER: &str = "HOPARLÖR";
/// Kind caption of a display output.
pub const KIND_DISPLAY: &str = "EKRAN";
/// The audio footer at rest.
pub const FOOTER_NORMAL: &str = "KAYNAĞA, SONRA ÇIKIŞA TIKLA";
/// The audio footer while a source is picked.
pub const FOOTER_PICKING: &str = "ÇIKIŞA TIKLA: YOL KURULUR · KAYNAĞA TEKRAR TIKLA: VAZGEÇ";
/// The audio footer of the outputs view.
pub const FOOTER_OUTPUTS: &str = "ÇIKIŞA TIKLA: VARSAYILAN OLUR";

/// Every own label above, for the upper-case checks.
pub const OWN_LABELS: [&str; 17] = [
    TAB_AUDIO,
    TAB_MEDIA,
    TAB_SHELF,
    TAB_SYSTEM,
    MUTED_LABEL,
    OFF_LABEL,
    DDC_OFF,
    SHELF_HINT,
    DEFAULT_CAPTION,
    OTHER_OUTPUTS,
    NO_OUTPUT,
    KIND_HEADPHONES,
    KIND_SPEAKER,
    KIND_DISPLAY,
    FOOTER_NORMAL,
    FOOTER_PICKING,
    FOOTER_OUTPUTS,
];

/// Upper-cases our own Turkish copy with Turkish rules: `i` becomes `İ`, `ı` becomes `I`, the
/// rest as [`str::to_uppercase`].
#[must_use]
pub fn tr_upper(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'i' => 'İ'.to_string(),
            'ı' => 'I'.to_string(),
            other => other.to_uppercase().collect(),
        })
        .collect()
}

#[cfg(test)]
mod tests;
