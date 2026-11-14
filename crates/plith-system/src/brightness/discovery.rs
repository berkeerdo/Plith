//! Which devices count, and how a snapshot follows writes and events. Pure.

use super::{BrightnessDevice, DeviceLevel};

/// The devices that answered a read, with the levels they reported.
pub struct Discovered {
    /// Answering devices, in candidate order.
    pub devices: Vec<Box<dyn BrightnessDevice>>,
    /// One level per kept device, same order.
    pub levels: Vec<DeviceLevel>,
}

/// Keeps the candidates that answer a read, in order. Dropped candidates are released here
/// (their `Drop` closes any handle).
///
/// A read is the entire capability test: `GetMonitorCapabilities` was measured returning false
/// with caps 0x0 on a monitor whose brightness reads and writes both work.
#[must_use]
pub fn keep_answering(candidates: Vec<Box<dyn BrightnessDevice>>) -> Discovered {
    let mut devices = Vec::new();
    let mut levels = Vec::new();
    for mut candidate in candidates {
        if let Some(percent) = candidate.read() {
            levels.push(DeviceLevel {
                id: candidate.id().to_owned(),
                name: candidate.name().to_owned(),
                percent: percent.min(100),
            });
            devices.push(candidate);
        }
    }
    Discovered { devices, levels }
}

/// Sets the level of the device `id`; `true` when it exists and the value changed.
pub fn apply_event(levels: &mut [DeviceLevel], id: &str, percent: u8) -> bool {
    let percent = percent.min(100);
    levels
        .iter_mut()
        .find(|level| level.id == id)
        .is_some_and(|level| std::mem::replace(&mut level.percent, percent) != percent)
}

/// Records a finished write: every device not in `failed` now reads `value`. `true` when any
/// level changed.
pub fn apply_write(levels: &mut [DeviceLevel], value: u8, failed: &[String]) -> bool {
    let value = value.min(100);
    let mut changed = false;
    for level in levels.iter_mut().filter(|l| !failed.contains(&l.id)) {
        changed |= std::mem::replace(&mut level.percent, value) != value;
    }
    changed
}
