//! Display brightness: every monitor whose level can be read, one shared percentage to set.
//!
//! Pure layer: [`step`] (arithmetic), [`writer`] (newest value wins, one write in flight),
//! [`discovery`] (keep only devices that answer a read) and [`wql`] (query text). Windows layer:
//! `ddc` (dxva2, external monitors), `wmi` (built-in panels) and `service` (worker thread).

pub mod discovery;
pub mod run_token;
pub mod step;
pub mod wql;
pub mod writer;

#[cfg(windows)]
mod ddc;
#[cfg(windows)]
mod listener;
mod panel_event;
#[cfg(windows)]
mod service;
#[cfg(test)]
mod tests;
#[cfg(windows)]
mod wmi;

pub use discovery::{Discovered, keep_answering};
pub use run_token::RunTokens;
#[cfg(windows)]
pub use service::BrightnessService;
pub use step::step_level;
pub use writer::{CoalescingWriter, WriteOutcome};

/// One display whose brightness can be read and set as a percentage (0 to 100).
///
/// Implementations are used from one thread at a time (the service worker), hence `&mut self`.
pub trait BrightnessDevice: Send {
    /// Stable identifier, unique among the devices of one run.
    fn id(&self) -> &str;

    /// Human readable name; defaults to the identifier.
    fn name(&self) -> &str {
        self.id()
    }

    /// The current level in percent. A successful read is the whole capability test: `None`
    /// means this device cannot be controlled.
    fn read(&mut self) -> Option<u8>;

    /// Sets the level in percent; `false` when the device refused or did not answer.
    fn write(&mut self, percent: u8) -> bool;
}

/// The level of one display at a moment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceLevel {
    /// Same as [`BrightnessDevice::id`].
    pub id: String,
    /// Same as [`BrightnessDevice::name`].
    pub name: String,
    /// Brightness, 0 to 100.
    pub percent: u8,
}

/// Whether the service started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// The worker is up. A machine with no controllable display is still `Running`, with an
    /// empty snapshot.
    Running,
    /// The worker could not start; the text says why.
    Unavailable(String),
}
