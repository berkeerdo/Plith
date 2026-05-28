using System.Windows;
using System.Windows.Threading;
using Plith.Views;

namespace Plith.Services;

/// <summary>
/// Owns the polling loop (Voicemeeter) and the push subscription (SMTC), funnels both into
/// a single OSD show pipeline, and routes media-button clicks to the SMTC session.
/// </summary>
public sealed class OsdOrchestrator : IDisposable
{
    private static readonly TimeSpan PollInterval = TimeSpan.FromMilliseconds(30);
    private static readonly TimeSpan ReconnectInterval = TimeSpan.FromSeconds(3);

    private readonly OsdWindow _osd;
    private readonly SettingsService _settings;
    private readonly Dispatcher _dispatcher;
    private readonly VoicemeeterClient _voicemeeter = new();
    private readonly MediaSessionClient _media = new();
    private readonly DispatcherTimer _pollTimer;
    private DateTime _nextReconnect = DateTime.MinValue;

    // Last-known values for the monitored bus. ShowOsd is gated on an actual change vs
    // these — count-based dirty suppression eats real user inputs that race the engine
    // seed pulses fired right after login. Value comparison is deterministic.
    private float? _lastGainDb;
    private bool? _lastMuted;

    private volatile bool _disposed;

    private TimeSpan VisibleFor => TimeSpan.FromMilliseconds(_settings.Current.ShowDurationMs);
    private int MonitoredBusIndex => _settings.Current.MonitoredBusIndex;

    public OsdOrchestrator(OsdWindow osd, SettingsService settings)
    {
        _osd = osd;
        _settings = settings;
        _dispatcher = osd.Dispatcher;
        _pollTimer = new DispatcherTimer(DispatcherPriority.Input) { Interval = PollInterval };
        _pollTimer.Tick += OnPollTick;
        _osd.MediaCommandInvoked += OnMediaCommandInvoked;

        // Re-baseline when the user switches monitored bus from the settings window — otherwise
        // the next dirty tick fires against the OLD bus's cached value and pops the OSD spuriously.
        _settings.Changed += OnSettingsChanged;
    }

    public void Start()
    {
        _pollTimer.Start();
        TryConnectVoicemeeter();

        _media.Changed += OnMediaChanged;
        _ = _media.StartAsync(); // fire-and-forget — failures degrade silently inside StartAsync
    }

    private void OnSettingsChanged(SettingsModel _)
    {
        // Force a baseline re-read so a bus change doesn't pop the OSD with the new bus's
        // current value as if it were a user-driven change.
        _lastGainDb = null;
        _lastMuted = null;
    }

    #region Voicemeeter polling

    private void OnPollTick(object? sender, EventArgs e)
    {
        if (!_voicemeeter.IsLoggedIn)
        {
            if (DateTime.UtcNow >= _nextReconnect) TryConnectVoicemeeter();
            return;
        }

        if (!_voicemeeter.ConsumeDirtyFlag()) return;

        if (!_voicemeeter.TryGetSnapshot(VoicemeeterRail.Bus, MonitoredBusIndex, out var snap))
            return;

        _osd.ViewModel.Apply(snap);

        // Only pop the OSD when the values actually changed from what we last saw — this
        // filters out the engine-seed pulses Voicemeeter fires after login (no value delta
        // there) while still catching every real user-driven change. The first read after
        // login establishes the baseline silently.
        bool isFirstRead = _lastGainDb is null;
        bool changed = isFirstRead
            || Math.Abs(_lastGainDb!.Value - snap.GainDb) > 0.001f
            || _lastMuted != snap.Muted;

        _lastGainDb = snap.GainDb;
        _lastMuted = snap.Muted;

        if (changed && !isFirstRead) _osd.ShowOsd(VisibleFor);
    }

    private void TryConnectVoicemeeter()
    {
        _nextReconnect = DateTime.UtcNow + ReconnectInterval;
        try
        {
            if (_voicemeeter.TryLogin())
            {
                // Force a baseline read on the next tick by clearing the cached values; the
                // very first snapshot then establishes the reference point with no OSD pop.
                _lastGainDb = null;
                _lastMuted = null;
            }
        }
        catch
        {
            // Voicemeeter DLL not present (yet). Will retry on next reconnect window.
        }
    }

    #endregion

    #region Media session push

    private void OnMediaChanged(MediaSnapshot snapshot)
    {
        // WinRT events fire on threadpool threads; bounce to the UI thread before touching VM/UI.
        // The _disposed guard prevents a queued callback from touching the OSD after teardown.
        if (!_dispatcher.CheckAccess())
        {
            _dispatcher.BeginInvoke(() => { if (!_disposed) OnMediaChanged(snapshot); });
            return;
        }
        if (_disposed) return;

        // Always update the VM silently so the card reflects current track whenever the OSD
        // does show. Auto-show on media is opt-in via settings — off by default since surfacing
        // a popup every Spotify advance is intrusive.
        _osd.ViewModel.Media.Apply(snapshot);

        if (_settings.Current.AutoShowOnMedia && snapshot.HasSession)
            _osd.ShowOsd(VisibleFor);
    }

    private void OnMediaCommandInvoked(object? sender, MediaCommand command)
    {
        _ = command switch
        {
            MediaCommand.SkipPrevious => _media.SkipPreviousAsync(),
            MediaCommand.TogglePlayPause => _media.TogglePlayPauseAsync(),
            MediaCommand.SkipNext => _media.SkipNextAsync(),
            _ => Task.FromResult(false),
        };
        // Pressing a button means the user is actively engaging — keep the OSD visible.
        _osd.ShowOsd(VisibleFor);
    }

    #endregion

    public void Dispose()
    {
        _disposed = true;
        _pollTimer.Stop();
        _settings.Changed -= OnSettingsChanged;
        _osd.MediaCommandInvoked -= OnMediaCommandInvoked;
        _media.Changed -= OnMediaChanged;
        _media.Dispose();
        _voicemeeter.Dispose();
    }
}
