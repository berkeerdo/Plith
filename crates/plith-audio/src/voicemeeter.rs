//! Pure Voicemeeter layer: types, parameter names and the login state machine.
//!
//! Nothing here touches Windows or the Voicemeeter DLL, so it is fully unit tested.

#[cfg(windows)]
mod api;
mod decode;
#[cfg(windows)]
mod dll;
mod layout;
#[cfg(windows)]
mod poll;
#[cfg(windows)]
mod service;
mod session;
#[cfg(test)]
mod tests;

pub use decode::{decode_label, install_dir_from_uninstall, level_channel, level_type};
pub use layout::{fallback_label, param_name};
#[cfg(windows)]
pub use service::{Status, VoicemeeterService};
pub use session::{DirtyOutcome, LoginSession};

/// Lowest gain the Remote API accepts, in dB.
pub const MIN_GAIN_DB: f32 = -60.0;
/// Highest gain the Remote API accepts, in dB.
pub const MAX_GAIN_DB: f32 = 12.0;
/// Floor used when converting a silent linear level to dB.
pub const LEVEL_FLOOR_DB: f32 = -90.0;

/// Which Voicemeeter edition is running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmKind {
    /// Voicemeeter (3 strips, 2 buses).
    Standard,
    /// Voicemeeter Banana (5 strips, 5 buses).
    Banana,
    /// Voicemeeter Potato (8 strips, 8 buses).
    Potato,
}

impl VmKind {
    /// Maps the type code reported by `VBVMR_GetVoicemeeterType`.
    #[must_use]
    pub fn from_api(code: i32) -> Option<VmKind> {
        match code {
            1 => Some(VmKind::Standard),
            2 => Some(VmKind::Banana),
            3 => Some(VmKind::Potato),
            _ => None,
        }
    }

    /// Number of input strips.
    #[must_use]
    pub fn strip_count(self) -> usize {
        match self {
            VmKind::Standard => 3,
            VmKind::Banana => 5,
            VmKind::Potato => 8,
        }
    }

    /// Number of output buses.
    #[must_use]
    pub fn bus_count(self) -> usize {
        match self {
            VmKind::Standard => 2,
            VmKind::Banana => 5,
            VmKind::Potato => 8,
        }
    }
}

/// Input strip or output bus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rail {
    /// An input strip.
    Strip,
    /// An output bus.
    Bus,
}

/// One strip or bus as last read from the engine.
#[derive(Debug, Clone, PartialEq)]
pub struct ChannelSnapshot {
    /// Strip or bus.
    pub rail: Rail,
    /// Zero-based index within the rail.
    pub index: usize,
    /// Display label (the engine label or a fallback).
    pub label: String,
    /// Gain in dB.
    pub gain_db: f32,
    /// Whether the channel is muted.
    pub muted: bool,
}

/// Full Voicemeeter state at one moment.
#[derive(Debug, Clone, PartialEq)]
pub struct VmSnapshot {
    /// Running edition.
    pub kind: VmKind,
    /// Input strips.
    pub strips: Vec<ChannelSnapshot>,
    /// Output buses.
    pub buses: Vec<ChannelSnapshot>,
}

/// Peak levels (linear, 0.0 to 1.0 and above on clipping) of the left and right channel of
/// every strip and bus. Read only; all zero while nothing is playing or Voicemeeter is absent.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VmLevels {
    /// Left and right peak per input strip.
    pub strips: Vec<[f32; 2]>,
    /// Left and right peak per output bus.
    pub buses: Vec<[f32; 2]>,
}

/// Clamps a gain to the accepted range; NaN becomes the minimum.
#[must_use]
pub fn clamp_gain(db: f32) -> f32 {
    if db.is_nan() {
        MIN_GAIN_DB
    } else {
        db.clamp(MIN_GAIN_DB, MAX_GAIN_DB)
    }
}

/// Converts a linear amplitude level to dB, floored at [`LEVEL_FLOOR_DB`].
#[must_use]
pub fn level_to_db(level: f32) -> f32 {
    if level.is_nan() || level <= 0.0 {
        return LEVEL_FLOOR_DB;
    }
    (20.0 * level.log10()).max(LEVEL_FLOOR_DB)
}
