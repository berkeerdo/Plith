using System.IO;
using Windows.Media.Control;
using Windows.Storage.Streams;

namespace Plith.Services;

public sealed record MediaSnapshot(
    string Title,
    string Artist,
    byte[]? ThumbnailBytes,
    bool IsPlaying,
    bool HasSession);

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

    /// <summary>Raised when the current session is swapped (e.g. user switches from Spotify to a browser tab).
    /// Subscribers may want to suppress the next <see cref="Changed"/> snapshot since it's just the new
    /// session's initial state, not a user-driven event.</summary>
    public event Action? SessionReplaced;

    /// <summary>AUMID of the app owning the current session, or empty when there is none.
    /// Used by FullscreenVideoWatcher to decide whether the foreground window is playing media.</summary>
    public string CurrentSourceAppUserModelId { get; private set; } = string.Empty;

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
    }

    private void DetachCurrent()
    {
        if (_currentSession is null) return;
        _currentSession.MediaPropertiesChanged -= OnSessionChanged;
        _currentSession.PlaybackInfoChanged -= OnSessionChanged;
        _currentSession = null;
    }

    private void OnSessionChanged(GlobalSystemMediaTransportControlsSession sender, object args) => ScheduleEmit();

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

        Changed?.Invoke(new MediaSnapshot(title, artist, thumb, playing, HasSession: true));
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
