using System.Windows;
using System.Windows.Threading;
using Plith.Views;

namespace Plith.Services;

/// <summary>
/// Polls Voicemeeter for parameter changes and shows the OSD when something interesting moves.
/// Polling cadence matches FancyOSD's defaults — 30 ms is well under perceptual threshold.
/// </summary>
public sealed class OsdOrchestrator : IDisposable
{
    private static readonly TimeSpan PollInterval = TimeSpan.FromMilliseconds(30);
    private static readonly TimeSpan ReconnectInterval = TimeSpan.FromSeconds(3);
    private static readonly TimeSpan VisibleFor = TimeSpan.FromSeconds(2);
    private const int MonitoredBusIndex = 0; // Bus A1 — the one bound to volume keys.

    // Voicemeeter fires the dirty flag 2–3 times in quick succession on login as the engine
    // seeds parameter state and the meter thread syncs. Yutmazsak boot'ta OSD anında pop'lar.
    private const int InitialDirtySuppressCount = 3;

    private readonly OsdWindow _osd;
    private readonly VoicemeeterClient _voicemeeter = new();
    private readonly DispatcherTimer _pollTimer;
    private DateTime _nextReconnect = DateTime.MinValue;
    private int _suppressDirtyCount = InitialDirtySuppressCount;

    public OsdOrchestrator(OsdWindow osd)
    {
        _osd = osd;
        _pollTimer = new DispatcherTimer(DispatcherPriority.Input)
        {
            Interval = PollInterval,
        };
        _pollTimer.Tick += OnPollTick;
    }

    public void Start()
    {
        _pollTimer.Start();
        TryConnect();
    }

    private void OnPollTick(object? sender, EventArgs e)
    {
        if (!_voicemeeter.IsLoggedIn)
        {
            if (DateTime.UtcNow >= _nextReconnect) TryConnect();
            return;
        }

        if (!_voicemeeter.ConsumeDirtyFlag()) return;

        if (!_voicemeeter.TryGetSnapshot(VoicemeeterRail.Bus, MonitoredBusIndex, out var snap))
            return;

        _osd.ViewModel.Apply(snap);

        if (_suppressDirtyCount > 0)
        {
            _suppressDirtyCount--;
            return;
        }

        _osd.ShowOsd(VisibleFor);
    }

    private void TryConnect()
    {
        _nextReconnect = DateTime.UtcNow + ReconnectInterval;
        try
        {
            if (_voicemeeter.TryLogin())
                _suppressDirtyCount = InitialDirtySuppressCount;
        }
        catch
        {
            // Voicemeeter DLL not present (yet). Will retry on next reconnect window.
        }
    }

    public void Dispose()
    {
        _pollTimer.Stop();
        _voicemeeter.Dispose();
    }
}
