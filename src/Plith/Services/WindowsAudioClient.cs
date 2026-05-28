using System.Threading;
using NAudio.CoreAudioApi;
using NAudio.CoreAudioApi.Interfaces;

namespace Plith.Services;

public sealed record WindowsAudioSnapshot(string DeviceLabel, float ScalarVolume, bool Muted);

/// <summary>
/// Wraps the Core Audio API default render endpoint via NAudio.
/// <see cref="AudioEndpointVolume.OnVolumeNotification"/> fires on a COM (MTA) thread,
/// so consumers must dispatch to the UI thread themselves.
/// Device-change tracking is deferred to Phase 4 — for now the snapshot is taken at
/// <see cref="Start"/> time and the user can restart Plith to follow a default-device switch.
/// </summary>
public sealed class WindowsAudioClient : IDisposable
{
    private MMDeviceEnumerator? _enumerator;
    private MMDevice? _device;
    private AudioEndpointVolume? _volume;
    private bool _disposed;

    /// <summary>True once <see cref="Start"/> succeeded in attaching to an endpoint.</summary>
    public bool IsAttached => _volume is not null;

    public event Action<WindowsAudioSnapshot>? Changed;

    public bool Start()
    {
        if (IsAttached) return true;
        try
        {
            _enumerator = new MMDeviceEnumerator();
            _device = _enumerator.GetDefaultAudioEndpoint(DataFlow.Render, Role.Multimedia);
            _volume = _device.AudioEndpointVolume;
            _volume.OnVolumeNotification += OnNotification;
            EmitSnapshot();
            return true;
        }
        catch
        {
            // No audio endpoint (headless box, broken driver). Caller will fall back or retry.
            Stop();
            return false;
        }
    }

    public void Stop()
    {
        // Null out the COM-visible fields BEFORE disposing — a notification already in flight
        // on the MTA thread may be between the unsubscribe call returning and OnNotification
        // entering EmitSnapshot. By clearing _volume first, EmitSnapshot's null check fires
        // immediately and never touches a disposed RCW (would otherwise throw a corrupted-
        // state exception that's not catchable by a plain try/catch on .NET 5+).
        var vol = Interlocked.Exchange(ref _volume, null);
        if (vol is not null)
        {
            try { vol.OnVolumeNotification -= OnNotification; } catch { }
        }
        var dev = Interlocked.Exchange(ref _device, null);
        var en = Interlocked.Exchange(ref _enumerator, null);
        dev?.Dispose();
        en?.Dispose();
    }

    private void OnNotification(AudioVolumeNotificationData data) => EmitSnapshot();

    private void EmitSnapshot()
    {
        var device = _device;
        var volume = _volume;
        if (device is null || volume is null) return;

        string label;
        float scalar;
        bool muted;
        try
        {
            label = device.FriendlyName ?? "Speakers";
            scalar = volume.MasterVolumeLevelScalar;   // 0..1, matches Windows' own percentage UI
            muted = volume.Mute;
        }
        catch
        {
            return; // device went away mid-read; ignore this pulse
        }

        Changed?.Invoke(new WindowsAudioSnapshot(label, scalar, muted));
    }

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        Stop();
    }
}
