using System.IO;
using Windows.Media.Control;
using Windows.Storage.Streams;

namespace Plith.Services;

/// <summary>
/// Where the current track is, and when that was last true.
///
/// A stamped reading rather than a live position, because that is what SMTC reports. Only ever
/// constructed with a positive <paramref name="Duration"/>: see
/// <see cref="MediaSessionClient.ReadTimeline"/>, which returns null otherwise, so no consumer
/// has to guard a division.
/// </summary>
public sealed record MediaTimeline(TimeSpan Position, TimeSpan Duration, DateTimeOffset LastUpdated);

public sealed record MediaSnapshot(
    string Title,
    string Artist,
    byte[]? ThumbnailBytes,
    bool IsPlaying,
    bool HasSession,
    // Defaulted so the five-argument construction in the no-session path and in the tests keeps
    // compiling and keeps meaning "no timeline".
    MediaTimeline? Timeline = null);

/// <summary>
/// Wraps Windows.Media.Control (SMTC) — the system-wide media session manager that
/// Spotify / Brave / YouTube / Edge etc. publish into. Events fire on threadpool
/// threads; the orchestrator marshals to the UI dispatcher itself rather than us
/// taking a dispatcher dependency here.
/// </summary>
public sealed class MediaSessionClient : IDisposable
{
    private GlobalSystemMediaTransportControlsSessionManager? _manager;
    private GlobalSystemMediaTransportControlsSession? _currentSession;
    private CancellationTokenSource _emitCts = new();
    private readonly object _emitLock = new();
    private bool _disposed;

    /// <summary>Raised after any change: track change, play/pause. Always carries a fresh snapshot.</summary>
    public event Action<MediaSnapshot>? Changed;

    /// <summary>
    /// Raised when the position moves, carrying only the timeline.
    ///
    /// Separate from <see cref="Changed"/> on purpose. Changed comes from ScheduleEmit, which
    /// re-reads the media properties AND re-downloads the album thumbnail, and
    /// TimelinePropertiesChanged fires about once a second on some sources. Subscribing the
    /// position to that path would download the artwork once per second.
    /// </summary>
    public event Action<MediaTimeline?>? TimelineChanged;

    /// <summary>Raised when the current session is swapped (e.g. user switches from Spotify to a browser tab).
    /// Subscribers may want to suppress the next <see cref="Changed"/> snapshot since it's just the new
    /// session's initial state, not a user-driven event.</summary>
    public event Action? SessionReplaced;

    /// <summary>AUMID of the app owning the current session, or empty when there is none.
    /// Used by FullscreenVideoWatcher to decide whether the foreground window is playing media.</summary>
    public string CurrentSourceAppUserModelId { get; private set; } = string.Empty;

    /// <summary>
    /// Bring the app that owns the current session to the front.
    ///
    /// shell:AppsFolder is the one launcher that takes an AUMID for both kinds of app: a Store
    /// package like Spotify has no path to start, and a desktop app registers an AUMID that
    /// resolves there too. Starting it through the shell rather than CreateProcess is what makes
    /// an already-running instance come forward instead of a second one opening.
    ///
    /// Returns false rather than throwing when there is no session or the id will not resolve —
    /// a click on the art is not worth taking the OSD down for.
    /// </summary>
    public bool TryOpenSourceApp()
    {
        var aumid = CurrentSourceAppUserModelId;
        if (string.IsNullOrWhiteSpace(aumid)) return false;

        try
        {
            System.Diagnostics.Process.Start(new System.Diagnostics.ProcessStartInfo
            {
                FileName = "explorer.exe",
                Arguments = @"shell:AppsFolder\" + aumid,
                UseShellExecute = true,
            });
            return true;
        }
        catch (Exception)
        {
            // Swallowed rather than logged: this class has no log of its own, and a click on the
            // album art is not worth taking the OSD down for. The caller gets false and leaves
            // the page as it was.
            return false;
        }
    }

    /// <summary>True while the current session reports Playing.</summary>
    public bool IsCurrentSessionPlaying { get; private set; }

    public async Task StartAsync()
    {
        try
        {
            _manager = await GlobalSystemMediaTransportControlsSessionManager.RequestAsync();
        }
        catch
        {
            // SMTC unavailable (rare — only on stripped server SKUs); silently degrade.
            return;
        }

        _manager.CurrentSessionChanged += OnCurrentSessionChanged;
        AttachCurrent();
        ScheduleEmit();
    }

    private void OnCurrentSessionChanged(GlobalSystemMediaTransportControlsSessionManager sender, CurrentSessionChangedEventArgs args)
    {
        DetachCurrent();
        AttachCurrent();
        SessionReplaced?.Invoke();
        ScheduleEmit();
    }

    private void AttachCurrent()
    {
        _currentSession = _manager?.GetCurrentSession();
        if (_currentSession is null) return;
        _currentSession.MediaPropertiesChanged += OnSessionChanged;
        _currentSession.PlaybackInfoChanged += OnSessionChanged;
        _currentSession.TimelinePropertiesChanged += OnTimelineChanged;
    }

    private void DetachCurrent()
    {
        if (_currentSession is null) return;
        _currentSession.MediaPropertiesChanged -= OnSessionChanged;
        _currentSession.PlaybackInfoChanged -= OnSessionChanged;
        _currentSession.TimelinePropertiesChanged -= OnTimelineChanged;
        _currentSession = null;
    }

    private void OnSessionChanged(GlobalSystemMediaTransportControlsSession sender, object args) => ScheduleEmit();

    private void OnTimelineChanged(GlobalSystemMediaTransportControlsSession sender,
                                   TimelinePropertiesChangedEventArgs args)
        => TimelineChanged?.Invoke(ReadTimeline(sender));

    /// <summary>
    /// The session's timeline, or null when there is nothing usable to draw.
    ///
    /// Null rather than a zero-length timeline for a live stream or a source that reports no end
    /// time: a bar of unknown length is a lie, and the page draws no bar for null.
    ///
    /// StartTime is subtracted rather than assumed to be zero, because it is not always zero for
    /// chaptered content. A missing LastUpdatedTime becomes now: left at default it is year 1,
    /// and the interpolation would then pin every bar to the end of its track.
    /// </summary>
    internal static MediaTimeline? ReadTimeline(GlobalSystemMediaTransportControlsSession session)
    {
        try
        {
            var t = session.GetTimelineProperties();
            if (t is null) return null;

            var duration = t.EndTime - t.StartTime;
            if (duration <= TimeSpan.Zero) return null;

            var position = t.Position - t.StartTime;
            var stamp = t.LastUpdatedTime == default ? DateTimeOffset.Now : t.LastUpdatedTime;
            return new MediaTimeline(position, duration, stamp);
        }
        catch
        {
            // Same contract as every other read in this class: a session that died mid-read costs
            // the caller a null, not an exception on a threadpool thread.
            return null;
        }
    }

    /// <summary>Cancels any in-flight <see cref="EmitSnapshotAsync"/> and fires a fresh one,
    /// so a stale read can never overwrite a newer one when events arrive in bursts.</summary>
    private void ScheduleEmit()
    {
        CancellationToken token;
        lock (_emitLock)
        {
            if (_disposed) return;
            _emitCts.Cancel();
            _emitCts.Dispose();
            _emitCts = new CancellationTokenSource();
            token = _emitCts.Token;
        }
        _ = EmitSnapshotAsync(token);
    }

    private async Task EmitSnapshotAsync(CancellationToken ct)
    {
        var session = _currentSession;
        if (session is null)
        {
            CurrentSourceAppUserModelId = string.Empty;
            IsCurrentSessionPlaying = false;
            if (!ct.IsCancellationRequested)
                Changed?.Invoke(new MediaSnapshot("", "", null, false, HasSession: false));
            return;
        }

        string title = "", artist = "";
        byte[]? thumb = null;
        try
        {
            var props = await session.TryGetMediaPropertiesAsync();
            if (ct.IsCancellationRequested) return;
            title = props?.Title ?? "";
            artist = props?.Artist ?? "";
            if (props?.Thumbnail is { } thumbRef)
                thumb = await ReadThumbnailAsync(thumbRef, ct);
        }
        catch
        {
            // Some sources momentarily return null props during transitions; treat as no-data.
        }
        if (ct.IsCancellationRequested) return;

        bool playing = false;
        try
        {
            playing = session.GetPlaybackInfo()?.PlaybackStatus
                == GlobalSystemMediaTransportControlsSessionPlaybackStatus.Playing;
        }
        catch { }

        if (ct.IsCancellationRequested) return;

        var aumid = string.Empty;
        try { aumid = session.SourceAppUserModelId ?? string.Empty; } catch { /* session died mid-read */ }

        CurrentSourceAppUserModelId = aumid;
        IsCurrentSessionPlaying = playing;

        // The timeline rides on the full snapshot too, so a subscriber that only listens to
        // Changed is never left without one.
        Changed?.Invoke(new MediaSnapshot(title, artist, thumb, playing, HasSession: true,
                                          ReadTimeline(session)));
    }

    private static async Task<byte[]?> ReadThumbnailAsync(IRandomAccessStreamReference thumbRef, CancellationToken ct)
    {
        try
        {
            using var winrtStream = await thumbRef.OpenReadAsync();
            if (ct.IsCancellationRequested) return null;
            using var stream = winrtStream.AsStreamForRead();
            using var ms = new MemoryStream();
            await stream.CopyToAsync(ms, ct);
            return ms.Length == 0 ? null : ms.ToArray();
        }
        catch
        {
            return null;
        }
    }

    public async Task<bool> TogglePlayPauseAsync()
    {
        var s = _currentSession;
        if (s is null) return false;
        try { return await s.TryTogglePlayPauseAsync(); }
        catch { return false; }
    }

    public async Task<bool> SkipNextAsync()
    {
        var s = _currentSession;
        if (s is null) return false;
        try { return await s.TrySkipNextAsync(); }
        catch { return false; }
    }

    public async Task<bool> SkipPreviousAsync()
    {
        var s = _currentSession;
        if (s is null) return false;
        try { return await s.TrySkipPreviousAsync(); }
        catch { return false; }
    }

    public void Dispose()
    {
        lock (_emitLock)
        {
            if (_disposed) return;
            _disposed = true;
            try { _emitCts.Cancel(); } catch { }
            _emitCts.Dispose();
        }

        DetachCurrent();
        if (_manager is not null)
        {
            try { _manager.CurrentSessionChanged -= OnCurrentSessionChanged; } catch { }
            _manager = null;
        }
    }
}
