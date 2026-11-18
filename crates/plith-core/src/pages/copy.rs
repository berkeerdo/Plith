//! The upper-casing helper for Turkish copy, and the two label constants the OSD still reads.
//! Everything else our own words say lives in the text table (`crate::lang`). Names that come
//! from Windows or Voicemeeter (devices, endpoints, strips, apps, track titles) never go through
//! [`tr_upper`]: the renderer upper-cases them invariantly.

// Task 4 of the R7 languages plan moves the OSD onto the table and removes both constants.
/// Tab label of the audio page, read by the OSD.
pub const TAB_AUDIO: &str = "SES";
/// A muted source or output, read by the OSD.
pub const MUTED_LABEL: &str = "SESSİZ";

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
