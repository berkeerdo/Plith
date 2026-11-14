//! System services: display brightness and suppression of the native Windows flyouts.
//!
//! [`brightness`] keeps its decisions (steps, write coalescing, discovery filtering, query
//! text) in a pure layer that is unit tested, and reaches Windows through a thin
//! `cfg(windows)` layer (dxva2 for external monitors, WMI for built-in panels).

pub mod brightness;

#[cfg(windows)]
pub use brightness::BrightnessService;
pub use brightness::{
    BrightnessDevice, CoalescingWriter, DeviceLevel, Status, WriteOutcome, step_level,
};
