//! External monitors over DDC/CI through dxva2. `cfg(windows)` only, thin.
//!
//! The physical-monitor handle is opened once and kept for the life of the device: a write was
//! measured at about 56 ms and that is the DDC/CI exchange itself, so reopening per write would
//! add to a cost that already limits how fast brightness can move. Dropping a device closes its
//! handle, which is how candidates that did not answer are released.

use windows::Win32::Devices::Display::{
    DestroyPhysicalMonitor, GetMonitorBrightness, GetNumberOfPhysicalMonitorsFromHMONITOR,
    GetPhysicalMonitorsFromHMONITOR, PHYSICAL_MONITOR, SetMonitorBrightness,
};
use windows::Win32::Foundation::{HANDLE, LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{EnumDisplayMonitors, HDC, HMONITOR};
use windows::core::BOOL;

use super::BrightnessDevice;
use super::step::{percent_to_raw, raw_to_percent};

/// One physical monitor reached over DDC/CI.
struct DdcDevice {
    handle: HANDLE,
    id: String,
    name: String,
    min: u32,
    max: u32,
}

// SAFETY: a physical-monitor handle from dxva2 is a plain token with no thread affinity; the
// service uses each device from one thread at a time.
unsafe impl Send for DdcDevice {}

impl BrightnessDevice for DdcDevice {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn read(&mut self) -> Option<u8> {
        let (mut min, mut current, mut max) = (0u32, 0u32, 0u32);
        // Deliberately not preceded by `GetMonitorCapabilities`: measured returning false with
        // caps 0x0 on a monitor whose brightness reads and writes both work.
        // SAFETY: `handle` is open until `Drop`; the three pointers address live locals.
        let ok = unsafe {
            GetMonitorBrightness(self.handle, &raw mut min, &raw mut current, &raw mut max)
        };
        if ok == 0 {
            return None;
        }
        (self.min, self.max) = (min, max);
        Some(raw_to_percent(min, current, max))
    }

    fn write(&mut self, percent: u8) -> bool {
        let raw = percent_to_raw(self.min, self.max, percent);
        // SAFETY: `handle` is open until `Drop`.
        unsafe { SetMonitorBrightness(self.handle, raw) != 0 }
    }
}

impl Drop for DdcDevice {
    fn drop(&mut self) {
        // SAFETY: `handle` came from `GetPhysicalMonitorsFromHMONITOR` and is destroyed once.
        let _ = unsafe { DestroyPhysicalMonitor(self.handle) };
    }
}

/// Collects each monitor handle into the `Vec<HMONITOR>` behind `data`.
unsafe extern "system" fn collect(
    monitor: HMONITOR,
    _dc: HDC,
    _rect: *mut RECT,
    data: LPARAM,
) -> BOOL {
    // SAFETY: `data` is the address of the vector `monitors` below, alive for the whole call.
    let list = unsafe { &mut *(data.0 as *mut Vec<HMONITOR>) };
    list.push(monitor);
    BOOL::from(true)
}

fn monitors() -> Vec<HMONITOR> {
    let mut list: Vec<HMONITOR> = Vec::new();
    // SAFETY: the callback only touches the vector passed as `data`, which outlives the call.
    let _ = unsafe {
        EnumDisplayMonitors(
            None,
            None,
            Some(collect),
            LPARAM(std::ptr::from_mut(&mut list) as isize),
        )
    };
    list
}

fn description(raw: &[u16]) -> String {
    let end = raw.iter().position(|&c| c == 0).unwrap_or(raw.len());
    String::from_utf16_lossy(&raw[..end]).trim().to_owned()
}

/// Every physical monitor attached now, answering or not; the caller filters by a read.
///
/// Ids are `description#n` with `n` counting across all monitors, because two identical
/// monitors report the same description.
pub(super) fn discover() -> Vec<Box<dyn BrightnessDevice>> {
    let mut found: Vec<Box<dyn BrightnessDevice>> = Vec::new();
    for monitor in monitors() {
        let mut count = 0u32;
        // SAFETY: `count` is a live local.
        if unsafe { GetNumberOfPhysicalMonitorsFromHMONITOR(monitor, &raw mut count) }.is_err()
            || count == 0
        {
            continue;
        }
        let mut physical: Vec<PHYSICAL_MONITOR> =
            (0..count).map(|_| PHYSICAL_MONITOR::default()).collect();
        // SAFETY: the slice has exactly `count` writable entries.
        if unsafe { GetPhysicalMonitorsFromHMONITOR(monitor, &mut physical) }.is_err() {
            continue;
        }
        for entry in &physical {
            // The struct is packed: copy the array out instead of referencing the field.
            let raw = entry.szPhysicalMonitorDescription;
            let text = description(&raw);
            let name = if text.is_empty() {
                "Display".to_owned()
            } else {
                text
            };
            found.push(Box::new(DdcDevice {
                handle: entry.hPhysicalMonitor,
                id: format!("{name}#{}", found.len()),
                name,
                min: 0,
                max: 100,
            }));
        }
    }
    found
}
