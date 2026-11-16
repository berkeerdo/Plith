//! The data model: one whole reading of the media state, assembled from independent reads.

use std::sync::Arc;

use crate::art::Artwork;
use crate::timeline::Timeline;

/// Whether SMTC can be used right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// The session manager is available (it may still have no session).
    Running,
    /// Not usable; the text says why.
    Unavailable(String),
}

/// Title, artist and artwork of the current track.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TrackFacts {
    /// Track title; empty when the source gives none.
    pub title: String,
    /// Artist; empty when the source gives none.
    pub artist: String,
    /// Encoded artwork bytes (PNG or JPEG as the source supplies them); `None` when absent.
    pub thumbnail: Option<Arc<[u8]>>,
}

/// Playback state and what the source accepts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent flags that mirror SMTC"
)]
pub struct PlaybackFacts {
    /// Whether the session reports Playing.
    pub playing: bool,
    /// Whether the source accepts a position write (asked of SMTC, never assumed).
    pub can_seek: bool,
    /// Whether skip next is enabled.
    pub can_next: bool,
    /// Whether skip previous is enabled.
    pub can_prev: bool,
}

/// One session in the list of all sessions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionInfo {
    /// App user model id of the owning app.
    pub source_id: String,
    /// Track title; empty when unknown.
    pub title: String,
    /// Whether the session reports Playing.
    pub playing: bool,
}

/// A failed or timed-out read must not erase a good value: keeps `previous` unless `read` has one.
/// The price is that after a failed read the previous title can stay briefly, even when the
/// source has already moved on, until the next read succeeds.
#[must_use]
pub fn keep_last_good<T>(previous: Option<T>, read: Option<T>) -> Option<T> {
    read.or(previous)
}

/// Everything known about the current media session at one moment.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent flags that mirror SMTC"
)]
pub struct MediaSnapshot {
    /// Whether there is a current session at all.
    pub has_session: bool,
    /// App user model id of the current session; empty without one.
    pub source_id: String,
    /// Track title.
    pub title: String,
    /// Artist.
    pub artist: String,
    /// Encoded artwork bytes.
    pub thumbnail: Option<Arc<[u8]>>,
    /// The thumbnail decoded to premultiplied BGRA on the worker; `None` when absent or undecodable.
    pub artwork: Option<Arc<Artwork>>,
    /// Whether the session reports Playing.
    pub playing: bool,
    /// Whether a seek is both accepted by the source and meaningful (there is a timeline).
    pub can_seek: bool,
    /// Whether skip next is enabled.
    pub can_next: bool,
    /// Whether skip previous is enabled.
    pub can_prev: bool,
    /// Position data; `None` for a live stream or a source without an end time.
    pub timeline: Option<Timeline>,
}

impl MediaSnapshot {
    /// The snapshot for "no session".
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    /// Replaces the decoded artwork (the worker decodes it, `assemble` stays pure).
    #[must_use]
    pub fn with_artwork(self, artwork: Option<Arc<Artwork>>) -> Self {
        Self { artwork, ..self }
    }

    /// Assembles a snapshot for a live session from independent reads. Any read that failed
    /// (the session died mid-read) is `None` and becomes an empty default rather than an error.
    /// Seeking needs a timeline, so `can_seek` is false without one.
    #[must_use]
    pub fn assemble(
        source_id: &str,
        track: Option<TrackFacts>,
        playback: Option<PlaybackFacts>,
        timeline: Option<Timeline>,
    ) -> Self {
        let track = track.unwrap_or_default();
        let playback = playback.unwrap_or_default();
        Self {
            has_session: true,
            source_id: source_id.to_owned(),
            title: track.title,
            artist: track.artist,
            thumbnail: track.thumbnail,
            artwork: None,
            playing: playback.playing,
            can_seek: playback.can_seek && timeline.is_some(),
            can_next: playback.can_next,
            can_prev: playback.can_prev,
            timeline,
        }
    }
}
