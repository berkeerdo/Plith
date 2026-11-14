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

pub mod flyout;
pub mod volume_keys;

#[cfg(windows)]
mod hook_thread;

#[cfg(windows)]
pub use flyout::FlyoutSuppressor;
pub use flyout::{HiddenRegistry, Match, Mode, SuppressionWindow, WindowFacts, should_hide};
#[cfg(windows)]
pub use volume_keys::VolumeKeyHook;
pub use volume_keys::handle_key_event;
