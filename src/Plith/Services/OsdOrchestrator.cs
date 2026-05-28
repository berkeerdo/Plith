using System.Windows;
using System.Windows.Threading;
using Plith.ViewModels;
using Plith.Views;

namespace Plith.Services;

/// <summary>
/// Owns the polling loop (Voicemeeter), the push subscription (SMTC), and the push
/// subscription (Windows Core Audio), and funnels whichever source the user picked
/// into a single OSD show pipeline. Routes media-button clicks back to the SMTC session.
/// </summary>
public sealed class OsdOrchestrator : IDisposable
{
    private static readonly TimeSpan PollInterval = TimeSpan.FromMilliseconds(30);
    private static readonly TimeSpan ReconnectInterval = TimeSpan.FromSeconds(3);

    private enum ActiveSource { None, Voicemeeter, Windows }

    private readonly OsdWindow _osd;
    private readonly SettingsService _settings;
    private readonly Dispatcher _dispatcher;
    private readonly VoicemeeterClient _voicemeeter = new();
    private readonly WindowsAudioClient _windowsAudio = new();
    private readonly MediaSessionClient _media = new();
    private readonly DispatcherTimer _pollTimer;
    private DateTime _nextReconnect = DateTime.MinValue;

    // Last-known values for the *active* source. Reset on every source swap so the next
    // snapshot establishes a baseline silently rather than popping with whatever the new
    // source's current value happens to be.
    private float? _lastNormalized;
    private bool? _lastMuted;
    private ActiveSource _activeSource = ActiveSource.None;

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
        _settings.Changed += OnSettingsChanged;
        _windowsAudio.Changed += OnWindowsAudioChanged;
    }

    public void Start()
    {
        _pollTimer.Start();
        TryConnectVoicemeeter();
        ReconcileActiveSource();

        _media.Changed += OnMediaChanged;
        _ = _media.StartAsync();
    }

    private void OnSettingsChanged(SettingsModel _)
    {
        // Mode may have changed (e.g. Auto → ForceWindows) — re-pick the active source.
        ReconcileActiveSource();
        // Bus index might have changed too — reset cache so the new bus's value is the baseline.
        _lastNormalized = null;
        _lastMuted = null;
    }

    /// <summary>
    /// Picks the active source based on user preference + Voicemeeter availability,
    /// and attaches / detaches the Windows client accordingly.
    /// </summary>
    private void ReconcileActiveSource()
    {
        var desired = _settings.Current.AudioSource switch
        {
            AudioSourceMode.ForceVoicemeeter => _voicemeeter.IsLoggedIn ? ActiveSource.Voicemeeter : ActiveSource.None,
            AudioSourceMode.ForceWindows => ActiveSource.Windows,
            _ /* Auto */                  => _voicemeeter.IsLoggedIn ? ActiveSource.Voicemeeter : ActiveSource.Windows,
        };

        if (desired == _activeSource) return;
        _lastNormalized = null;
        _lastMuted = null;

        if (desired == ActiveSource.Windows)
        {
            // Only commit the source change if NAudio actually attached. A failed Start() leaves
            // _activeSource at None so the next Reconcile re-tries instead of getting stuck
            // permanently silent because the early-return on the next call sees no change.
            if (_windowsAudio.IsAttached || _windowsAudio.Start())
                _activeSource = ActiveSource.Windows;
            else
                _activeSource = ActiveSource.None;
        }
        else
        {
            if (_windowsAudio.IsAttached) _windowsAudio.Stop();
            _activeSource = desired;
        }
    }

    #region Voicemeeter polling

    private void OnPollTick(object? sender, EventArgs e)
    {
        if (_activeSource == ActiveSource.Voicemeeter && _voicemeeter.IsLoggedIn)
        {
            // VBVMR_IsParametersDirty is an edge-triggered self-clearing latch — consume it
            // while VM is still the active source. ConsumeDirtyFlag also detects a dead engine
            // (negative return) and flips _loggedIn to false, which the Reconcile below catches.
            bool dirty = _voicemeeter.ConsumeDirtyFlag();
            if (dirty && _voicemeeter.TryGetSnapshot(VoicemeeterRail.Bus, MonitoredBusIndex, out var snap))
                HandleVoicemeeterChange(snap);
        }

        // Reconcile after the consume call so a VM-just-died transition flips us to the Windows
        // fallback on the same tick.
        ReconcileActiveSource();

        if (!_voicemeeter.IsLoggedIn && DateTime.UtcNow >= _nextReconnect)
            TryConnectVoicemeeter();
    }

    private void TryConnectVoicemeeter()
    {
        _nextReconnect = DateTime.UtcNow + ReconnectInterval;
        try
        {
            if (_voicemeeter.TryLogin())
            {
                _lastNormalized = null;
                _lastMuted = null;
                // VM just came online — in Auto mode, switch over to it.
                ReconcileActiveSource();
            }
        }
        catch
        {
            // Voicemeeter DLL not present (yet). Will retry on next reconnect window.
        }
    }

    #endregion

    #region Windows audio push

    private void OnWindowsAudioChanged(WindowsAudioSnapshot snapshot)
    {
        // NAudio's OnVolumeNotification fires on a COM (MTA) thread; bounce to the dispatcher.
        if (!_dispatcher.CheckAccess())
        {
            _dispatcher.BeginInvoke(() => { if (!_disposed) OnWindowsAudioChanged(snapshot); });
            return;
        }
        if (_disposed) return;
        if (_activeSource != ActiveSource.Windows) return;

        // Windows reports a 0..1 scalar; show it as the matching percent. InvariantCulture
        // keeps the digit-only output consistent across locales.
        var text = (snapshot.ScalarVolume * 100).ToString("0",
            System.Globalization.CultureInfo.InvariantCulture) + "%";
        HandleValueChange(snapshot.DeviceLabel, snapshot.ScalarVolume, text, snapshot.Muted);
    }

    #endregion

    #region Common change handling

    private void HandleVoicemeeterChange(VoicemeeterParameterSnapshot snap)
    {
        double normalized = (Math.Clamp(snap.GainDb, OsdViewModel.VoicemeeterMinDb, OsdViewModel.VoicemeeterMaxDb)
                            - OsdViewModel.VoicemeeterMinDb)
                          / (OsdViewModel.VoicemeeterMaxDb - OsdViewModel.VoicemeeterMinDb);
        // Invariant-culture formatting so the dB readout stays "0.0 dB" everywhere — the
        // CurrentCulture variant surfaces "0,0 dB" on tr-TR/de-DE/fr-FR machines, which
        // violates audio-engineering convention.
        string text = snap.GainDb.ToString("+0.0;-0.0;0.0",
            System.Globalization.CultureInfo.InvariantCulture) + " dB";
        HandleValueChange(snap.Label, normalized, text, snap.Muted);
    }

    /// <summary>
    /// Applies a normalized 0..1 value + a pre-formatted display string. Suppresses the first
    /// read after a source attach so the OSD doesn't pop on a baseline; pops on any real change.
    /// </summary>
    private void HandleValueChange(string label, double normalized, string text, bool muted)
    {
        bool isFirstRead = _lastNormalized is null;
        bool changed = isFirstRead
            || Math.Abs(_lastNormalized!.Value - normalized) > 0.0005
            || _lastMuted != muted;

        _lastNormalized = (float)normalized;
        _lastMuted = muted;

        _osd.ViewModel.Apply(label, normalized, text, muted);
        if (changed && !isFirstRead) _osd.ShowOsd(VisibleFor);
    }

    #endregion

    #region Media session push

    private void OnMediaChanged(MediaSnapshot snapshot)
    {
        if (!_dispatcher.CheckAccess())
        {
            _dispatcher.BeginInvoke(() => { if (!_disposed) OnMediaChanged(snapshot); });
            return;
        }
        if (_disposed) return;

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
        _windowsAudio.Changed -= OnWindowsAudioChanged;
        _media.Dispose();
        _windowsAudio.Dispose();
        _voicemeeter.Dispose();
    }
}
