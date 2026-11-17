//! The OSD trigger diff: which volume or brightness changes deserve an overlay.

use crate::pages::PageId;
use crate::pages::audio::AudioFacts;
use crate::pages::copy::TAB_AUDIO;
use crate::pages::system::SystemFacts;

/// The longest label, in bytes, the OSD protocol carries.
pub const MAX_LABEL_BYTES: usize = 64;

/// Which meter a level belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OsdMeter {
    /// The attached endpoint's volume.
    Volume,
    /// The monitor brightness.
    Brightness,
}

/// One meter reading, ready to show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OsdLevel {
    /// Which meter.
    pub meter: OsdMeter,
    /// The level, 0 to 100.
    pub percent: u8,
    /// Whether the meter is muted (always false for brightness).
    pub muted: bool,
    /// The device name, at most [`MAX_LABEL_BYTES`] bytes.
    pub label: String,
}

/// Both meters at one moment; `None` when a meter is unavailable.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OsdReading {
    /// The volume meter.
    pub volume: Option<OsdLevel>,
    /// The brightness meter.
    pub brightness: Option<OsdLevel>,
}

/// Cuts `text` to at most [`MAX_LABEL_BYTES`] bytes at a char boundary.
fn cut_label(text: &str) -> String {
    let mut end = text.len().min(MAX_LABEL_BYTES);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_string()
}

/// Rounds a 0 to 1 scalar to a percentage, clamped to 0 to 100.
fn percent_of(scalar: f32) -> u8 {
    // Clamped to 0..=100 before the cast, so neither truncation nor sign loss can occur.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let percent = (scalar * 100.0).round().clamp(0.0, 100.0) as u8;
    percent
}

/// Reads both meters from the page facts.
#[must_use]
pub fn reading(audio: &AudioFacts, system: &SystemFacts) -> OsdReading {
    let volume = audio.attached.map(|(scalar, muted)| {
        let name = audio
            .default_id
            .as_ref()
            .and_then(|id| audio.endpoints.iter().find(|endpoint| &endpoint.id == id))
            .map_or(TAB_AUDIO, |endpoint| endpoint.name.as_str());
        OsdLevel {
            meter: OsdMeter::Volume,
            percent: percent_of(scalar),
            muted,
            label: cut_label(name),
        }
    });
    let brightness = system.brightness.as_ref().map(|(name, percent)| OsdLevel {
        meter: OsdMeter::Brightness,
        percent: (*percent).min(100),
        muted: false,
        label: cut_label(name),
    });
    OsdReading { volume, brightness }
}

/// True when both sides hold the meter and its percent or mute differ.
fn moved(prev: Option<&OsdLevel>, next: Option<&OsdLevel>) -> Option<OsdLevel> {
    match (prev, next) {
        (Some(a), Some(b)) if a.percent != b.percent || a.muted != b.muted => Some(b.clone()),
        _ => None,
    }
}

/// The levels to show for the step from `prev` to `next`.
///
/// A meter that just appeared is not a change. A meter whose own page is open stays quiet.
#[must_use]
pub fn changes(prev: &OsdReading, next: &OsdReading, open: Option<PageId>) -> Vec<OsdLevel> {
    let mut fired = Vec::new();
    if open != Some(PageId::Audio) {
        fired.extend(moved(prev.volume.as_ref(), next.volume.as_ref()));
    }
    if open != Some(PageId::System) {
        fired.extend(moved(prev.brightness.as_ref(), next.brightness.as_ref()));
    }
    fired
}

pub mod policy;

#[cfg(test)]
mod tests;
