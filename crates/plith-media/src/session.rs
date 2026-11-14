//! Thin SMTC reads and commands. Every function tolerates a session that died mid-call: it
//! returns `None` or `false`, never panics. Async `WinRT` operations are waited on here with a
//! cap, and only ever from the service's worker thread.

use std::sync::mpsc::channel;
use std::time::{Duration, SystemTime};

use windows::Foundation::TimeSpan;
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession as Session,
    GlobalSystemMediaTransportControlsSessionManager as Manager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as PlaybackStatus,
};
use windows::Storage::Streams::{DataReader, IRandomAccessStreamReference};
use windows::core::{Interface, RuntimeType};
use windows_future::{AsyncOperationCompletedHandler, IAsyncInfo, IAsyncOperation};

use crate::snapshot::{PlaybackFacts, SessionInfo, TrackFacts};
use crate::timeline::{Timeline, datetime_to_system_time, normalize, ticks_to_duration};

/// How long a worker waits for any one `WinRT` operation.
pub(crate) const OP_CAP: Duration = Duration::from_secs(2);

/// Largest artwork read; anything bigger is dropped rather than copied.
const MAX_THUMBNAIL_BYTES: u64 = 8 * 1024 * 1024;

/// Waits for `op` for at most `cap`; a slow op is cancelled and gives `None`.
pub(crate) fn wait<T: RuntimeType + 'static>(op: &IAsyncOperation<T>, cap: Duration) -> Option<T> {
    let (tx, rx) = channel();
    // The handler runs on a threadpool thread and only signals the channel.
    op.SetCompleted(&AsyncOperationCompletedHandler::new(move |_, _| {
        let _ = tx.send(());
        Ok(())
    }))
    .ok()?;
    if rx.recv_timeout(cap).is_err() {
        if let Ok(info) = op.cast::<IAsyncInfo>() {
            let _ = info.Cancel();
        }
        return None;
    }
    op.GetResults().ok()
}

/// Asks Windows for the session manager; `Err` says why SMTC is unavailable.
pub(crate) fn request_manager() -> Result<Manager, String> {
    let op = Manager::RequestAsync().map_err(|e| format!("SMTC is not available: {e}"))?;
    wait(&op, OP_CAP).ok_or_else(|| "SMTC did not answer in time".to_owned())
}

/// The app user model id of a session; empty when it died.
pub(crate) fn source_id(session: &Session) -> String {
    session
        .SourceAppUserModelId()
        .map(|id| id.to_string())
        .unwrap_or_default()
}

/// Playback status and capabilities. `IsPlaybackPositionEnabled` is asked, not assumed.
pub(crate) fn read_playback(session: &Session) -> Option<PlaybackFacts> {
    let info = session.GetPlaybackInfo().ok()?;
    let controls = info.Controls().ok()?;
    Some(PlaybackFacts {
        playing: info.PlaybackStatus().ok()? == PlaybackStatus::Playing,
        can_seek: controls.IsPlaybackPositionEnabled().unwrap_or(false),
        can_next: controls.IsNextEnabled().unwrap_or(false),
        can_prev: controls.IsPreviousEnabled().unwrap_or(false),
    })
}

/// The position data, or `None` for no usable timeline (see [`normalize`]).
pub(crate) fn read_timeline(session: &Session) -> Option<Timeline> {
    let props = session.GetTimelineProperties().ok()?;
    let span =
        |value: windows::core::Result<TimeSpan>| ticks_to_duration(value.map_or(0, |t| t.Duration));
    let stamp = props
        .LastUpdatedTime()
        .ok()
        .and_then(|t| datetime_to_system_time(t.UniversalTime));
    normalize(
        span(props.StartTime()),
        span(props.EndTime()),
        span(props.Position()),
        stamp,
        SystemTime::now(),
    )
}

/// Title, artist and artwork. This is the expensive read (it downloads the thumbnail), so the
/// worker calls it only on a media-properties change, never on the timeline event.
pub(crate) fn read_track(session: &Session) -> Option<TrackFacts> {
    let props = wait(&session.TryGetMediaPropertiesAsync().ok()?, OP_CAP)?;
    let text = |value: windows::core::Result<windows::core::HSTRING>| {
        value.map(|s| s.to_string()).unwrap_or_default()
    };
    let thumbnail = props
        .Thumbnail()
        .ok()
        .and_then(|reference| read_thumbnail(&reference));
    Some(TrackFacts {
        title: text(props.Title()),
        artist: text(props.Artist()),
        thumbnail,
    })
}

fn read_thumbnail(reference: &IRandomAccessStreamReference) -> Option<Vec<u8>> {
    let stream = wait(&reference.OpenReadAsync().ok()?, OP_CAP)?;
    let size = stream.Size().ok()?;
    if size == 0 || size > MAX_THUMBNAIL_BYTES {
        return None;
    }
    let size = u32::try_from(size).ok()?;
    let reader = DataReader::CreateDataReader(&stream).ok()?;
    let loaded = wait(&reader.LoadAsync(size).ok()?, OP_CAP)?;
    let mut bytes = vec![0u8; usize::try_from(loaded).ok()?];
    reader.ReadBytes(&mut bytes).ok()?;
    (!bytes.is_empty()).then_some(bytes)
}

/// Every session with its source, title and playing state.
pub(crate) fn list_sessions(manager: &Manager) -> Vec<SessionInfo> {
    let Ok(sessions) = manager.GetSessions() else {
        return Vec::new();
    };
    sessions
        .into_iter()
        .map(|session| SessionInfo {
            source_id: source_id(&session),
            title: read_track_title(&session),
            playing: read_playback(&session).is_some_and(|p| p.playing),
        })
        .collect()
}

fn read_track_title(session: &Session) -> String {
    session
        .TryGetMediaPropertiesAsync()
        .ok()
        .and_then(|op| wait(&op, OP_CAP))
        .and_then(|props| props.Title().ok())
        .map(|title| title.to_string())
        .unwrap_or_default()
}

/// A transport command; each maps to one `Try...Async` call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Command {
    TogglePlayPause,
    Next,
    Previous,
    Seek(Duration),
}

/// Runs a command on `session`; `false` when the session is gone, the source refuses or the
/// call takes longer than the cap.
pub(crate) fn run_command(session: &Session, command: Command) -> bool {
    let op = match command {
        Command::TogglePlayPause => session.TryTogglePlayPauseAsync(),
        Command::Next => session.TrySkipNextAsync(),
        Command::Previous => session.TrySkipPreviousAsync(),
        Command::Seek(to) => {
            // The caller's position has the track start subtracted; SMTC wants it added back.
            let start = session
                .GetTimelineProperties()
                .and_then(|t| t.StartTime())
                .map_or(0, |t| t.Duration);
            let ticks = i64::try_from(to.as_nanos() / 100).unwrap_or(i64::MAX);
            session.TryChangePlaybackPositionAsync(ticks.saturating_add(start.max(0)))
        }
    };
    op.ok().and_then(|op| wait(&op, OP_CAP)).unwrap_or(false)
}
