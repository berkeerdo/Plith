//! Pure helpers for the Windows layer: install directory parsing, label decoding and
//! `VBVMR_GetLevel` channel mapping.

use super::{Rail, VmKind};

/// Channels per virtual strip (VAIO, AUX, VAIO3) and per bus in the level table.
const WIDE_CHANNELS: usize = 8;

/// Turns the registry `UninstallString` into the directory that holds the Remote API DLL.
///
/// Accepts a quoted path, a bare path, and either followed by arguments (`setup.exe /u`).
/// Returns `None` when no directory part can be found.
#[must_use]
pub fn install_dir_from_uninstall(uninstall: &str) -> Option<String> {
    let text = uninstall.trim();
    let path = if let Some(rest) = text.strip_prefix('"') {
        rest.split('"').next().unwrap_or("")
    } else {
        // Unquoted: cut at the end of the executable name so trailing arguments are dropped.
        let lower = text.to_ascii_lowercase();
        match lower.find(".exe") {
            Some(at) => &text[..at + 4],
            None => text,
        }
    };
    let cut = path.rfind(['\\', '/'])?;
    let dir = path[..cut].trim();
    if dir.is_empty() {
        None
    } else {
        Some(dir.to_owned())
    }
}

/// Decodes the ANSI buffer `VBVMR_GetParameterStringA` filled: stops at the first NUL, reads
/// UTF-8 when valid and falls back to Latin-1 otherwise.
#[must_use]
pub fn decode_label(buffer: &[u8]) -> String {
    let end = buffer.iter().position(|&b| b == 0).unwrap_or(buffer.len());
    let bytes = &buffer[..end];
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_owned(),
        Err(_) => bytes.iter().map(|&b| char::from(b)).collect(),
    }
}

/// Number of two-channel hardware strips before the wide virtual strips.
fn hardware_strips(kind: VmKind) -> usize {
    match kind {
        VmKind::Standard => 2,
        VmKind::Banana => 3,
        VmKind::Potato => 5,
    }
}

/// First channel (left side of the pair) of a strip or bus in the `VBVMR_GetLevel` table.
///
/// Hardware strips use two channels each, the virtual strips that follow use eight, and every
/// bus uses eight. Returns `None` when `index` is out of range for the edition.
#[must_use]
pub fn level_channel(kind: VmKind, rail: Rail, index: usize) -> Option<usize> {
    match rail {
        Rail::Strip => {
            if index >= kind.strip_count() {
                return None;
            }
            let hardware = hardware_strips(kind);
            if index < hardware {
                Some(index * 2)
            } else {
                Some(hardware * 2 + (index - hardware) * WIDE_CHANNELS)
            }
        }
        Rail::Bus => (index < kind.bus_count()).then_some(index * WIDE_CHANNELS),
    }
}

/// `nType` argument for `VBVMR_GetLevel`: 1 is post-fader input, 3 is output.
#[must_use]
pub fn level_type(rail: Rail) -> i32 {
    match rail {
        Rail::Strip => 1,
        Rail::Bus => 3,
    }
}
