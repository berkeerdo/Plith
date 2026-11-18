//! The upper-casing helper for Turkish copy. Everything our own words say lives in the text
//! table (`crate::lang`). Names that come from Windows or Voicemeeter (devices, endpoints,
//! strips, apps, track titles) never go through [`tr_upper`]: the renderer upper-cases them
//! invariantly.

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
