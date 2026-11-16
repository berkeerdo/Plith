//! Media services: SMTC sessions, seek and the data behind the output picker.
//!
//! The pure layer ([`timeline`], [`snapshot`], [`coalesce`]) holds every decision and is unit
//! tested. The Windows layer (`session`, `service`) is thin: it reads System Media Transport
//! Controls on one worker thread and publishes whole [`MediaSnapshot`]s.

pub mod art;
pub mod coalesce;
#[cfg(windows)]
mod com;
#[cfg(windows)]
mod pending;
#[cfg(windows)]
mod service;
#[cfg(windows)]
mod session;
pub mod snapshot;
#[cfg(test)]
mod tests;
pub mod timeline;
#[cfg(windows)]
mod waker;

pub use art::Artwork;
pub use coalesce::Generation;
#[cfg(windows)]
pub use service::MediaService;
pub use snapshot::{MediaSnapshot, PlaybackFacts, SessionInfo, Status, TrackFacts};
pub use timeline::{Timeline, normalize, position_at};
