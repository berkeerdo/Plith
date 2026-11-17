//! What kind of device an output endpoint is: headphones, a speaker or a display. The kind picks
//! the glyph and the caption of an output row. Pure: the app passes the raw Windows form factor
//! and names, and no Windows type enters this crate.

use crate::pages::copy;

/// `EndpointFormFactor` value for headphones.
const FORM_HEADPHONES: u32 = 3;
/// `EndpointFormFactor` value for a headset.
const FORM_HEADSET: u32 = 5;
/// `EndpointFormFactor` value for a digital audio display (HDMI or `DisplayPort`).
const FORM_DISPLAY: u32 = 9;

/// Name fragments (normalised: lower case, dotted and dotless i folded to `i`) of headphones.
const HEADPHONE_WORDS: [&str; 6] = [
    "headset",
    "headphone",
    "kulaklik",
    "earphone",
    "buds",
    "airpods",
];
/// Name fragments of a display output.
const DISPLAY_WORDS: [&str; 2] = ["hdmi", "displayport"];

/// The kind of an output endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DeviceKind {
    /// Headphones or a headset.
    Headphones,
    /// Loudspeakers, and anything not recognised.
    #[default]
    Speaker,
    /// A monitor or TV reached over HDMI or `DisplayPort`.
    Display,
}

impl DeviceKind {
    /// The upper-case caption shown under the name.
    #[must_use]
    pub fn caption(self) -> &'static str {
        match self {
            DeviceKind::Headphones => copy::KIND_HEADPHONES,
            DeviceKind::Speaker => copy::KIND_SPEAKER,
            DeviceKind::Display => copy::KIND_DISPLAY,
        }
    }
}

/// The kind of an endpoint: the Windows form factor decides first (headphones, headset,
/// display interface), then keywords in the names, else a speaker.
#[must_use]
pub fn device_kind(form_factor: Option<u32>, names: &[&str]) -> DeviceKind {
    match form_factor {
        Some(FORM_HEADPHONES | FORM_HEADSET) => return DeviceKind::Headphones,
        Some(FORM_DISPLAY) => return DeviceKind::Display,
        _ => {}
    }
    let folded: Vec<String> = names.iter().map(|n| fold(n)).collect();
    let has = |words: &[&str]| {
        folded
            .iter()
            .any(|name| words.iter().any(|w| name.contains(w)))
    };
    if has(&HEADPHONE_WORDS) {
        DeviceKind::Headphones
    } else if has(&DISPLAY_WORDS) {
        DeviceKind::Display
    } else {
        DeviceKind::Speaker
    }
}

/// Lower-cases with the Turkish pairs folded: `I`, `İ` and `ı` all read as `i`, so `KULAKLIK`,
/// `Kulaklık` and `kulaklik` match one keyword.
fn fold(name: &str) -> String {
    name.chars()
        .flat_map(|c| match c {
            'I' | '\u{130}' | '\u{131}' => 'i'.to_lowercase().collect::<Vec<_>>(),
            other => other.to_lowercase().collect(),
        })
        .collect()
}

#[cfg(test)]
mod tests;
