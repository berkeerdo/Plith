using System.Threading;
using NAudio.CoreAudioApi;
using NAudio.CoreAudioApi.Interfaces;

namespace Plith.Services;

public sealed record WindowsAudioSnapshot(string DeviceLabel, float ScalarVolume, bool Muted);

/// <summary>
/// Wraps the Core Audio API default render endpoint via NAudio.
/// <see cref="AudioEndpointVolume.OnVolumeNotification"/> fires on a COM (MTA) thread,
/// so consumers must dispatch to the UI thread themselves.
/// Implements <see cref="IMMNotificationClient"/> so a default-device swap (user plugs in
/// headphones, switches output) triggers a transparent reattach to the new endpoint.
/// </summary>
public sealed class WindowsAudioClient : IDisposable, IMMNotificationClient
{
    private readonly DiagnosticLog? _log;
    private MMDeviceEnumerator? _enumerator;
    private MMDevice? _device;
    private AudioEndpointVolume? _volume;
    private bool _disposed;

    public WindowsAudioClient(DiagnosticLog? log = null)
    {
        _log = log;
    }

    // Serializes the Detach + Attach sequences. COM can fire two OnDefaultDeviceChanged
    // callbacks on different MTA threads in quick succession (rapid headset plug/unplug),
    // and without this lock the second's Detach can null out fields under the first's
    // half-finished Attach — leaking subscriptions and disposing devices the other thread
    // is still reading. Stop() takes the same lock so Dispose can't race a callback either.
    private readonly object _attachLock = new();

    /// <summary>True once <see cref="Start"/> succeeded in attaching to an endpoint.</summary>
    public bool IsAttached => _volume is not null;

    public event Action<WindowsAudioSnapshot>? Changed;

    public bool Start()
    {
        lock (_attachLock)
        {
            if (IsAttached) { _log?.Info("WindowsAudio", "Start: already attached"); return true; }
            try
            {
                _log?.Info("WindowsAudio", "Start: creating MMDeviceEnumerator + RegisterEndpointNotificationCallback");
                _enumerator = new MMDeviceEnumerator();
                _enumerator.RegisterEndpointNotificationCallback(this);
                AttachToCurrentDefault();
                _log?.Info("WindowsAudio", $"Start: attached to '{_device?.FriendlyName ?? "?"}'");
            }
            catch (Exception ex)
            {
                _log?.Error("WindowsAudio", $"Start failed: {ex.GetType().Name}: {ex.Message}");
                // No audio endpoint (headless box, broken driver). Caller will fall back or retry.
                StopInternal();
                return false;
            }
        }
        EmitSnapshot();
        return true;
    }

    private void AttachToCurrentDefault()
    {
        var en = _enumerator;
        if (en is null) return;
        _device = en.GetDefaultAudioEndpoint(DataFlow.Render, Role.Multimedia);
        _volume = _device.AudioEndpointVolume;
        _volume.OnVolumeNotification += OnNotification;
    }

    private void DetachFromCurrentDevice()
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
        dev?.Dispose();
    }

    public void Stop()
    {
        lock (_attachLock) StopInternal();
    }

    private void StopInternal()
    {
        DetachFromCurrentDevice();
        var en = Interlocked.Exchange(ref _enumerator, null);
        if (en is not null)
        {
            try { en.UnregisterEndpointNotificationCallback(this); } catch { }
            en.Dispose();
        }
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

    #region IMMNotificationClient — default-device tracking

    public void OnDefaultDeviceChanged(DataFlow flow, Role role, string defaultDeviceId)
    {
        if (flow != DataFlow.Render || role != Role.Multimedia) return;
        _log?.Info("WindowsAudio", $"OnDefaultDeviceChanged: new device id={defaultDeviceId}");

        lock (_attachLock)
        {
            if (_disposed || _enumerator is null) return;

            try
            {
                DetachFromCurrentDevice();
                AttachToCurrentDefault();
            }
            catch
            {
                // The new device evaporated mid-swap (rare but possible). Drop everything so the
                // orchestrator's reconcile pass can re-try by calling Start again.
                DetachFromCurrentDevice();
                return;
            }
        }
        EmitSnapshot();
    }

    public void OnDeviceStateChanged(string deviceId, DeviceState newState) { }
    public void OnDeviceAdded(string pwstrDeviceId) { }
    public void OnDeviceRemoved(string deviceId) { }
    public void OnPropertyValueChanged(string pwstrDeviceId, PropertyKey key) { }

    #endregion

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        Stop();
    }
}
