using System.Threading;
using NAudio.CoreAudioApi;
using NAudio.CoreAudioApi.Interfaces;

namespace Plith.Services;

public sealed record WindowsAudioSnapshot(string DeviceLabel, float ScalarVolume, bool Muted);

/// <summary>A single active render endpoint the user can pick in Settings.</summary>
public sealed record WindowsAudioEndpointInfo(string Id, string FriendlyName);

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

    // When non-null and non-empty, the client pins to this specific endpoint by ID and
    // ignores OS default-device swaps. When null / empty, it follows Windows' default
    // render endpoint (original behavior).
    private string? _targetEndpointId;

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

        // Pinned mode: try the user-selected endpoint first. If it isn't active anymore
        // (unplugged, disabled), silently fall through to the default endpoint so the OSD
        // keeps working instead of going dark until the user re-picks in Settings.
        if (!string.IsNullOrEmpty(_targetEndpointId))
        {
            try
            {
                var pinned = en.GetDevice(_targetEndpointId);
                if (pinned is not null && pinned.State == DeviceState.Active)
                {
                    _device = pinned;
                    _volume = pinned.AudioEndpointVolume;
                    _volume.OnVolumeNotification += OnNotification;
                    _log?.Info("WindowsAudio", $"Attached to pinned endpoint '{pinned.FriendlyName}'");
                    return;
                }
                _log?.Warn("WindowsAudio", $"Pinned endpoint '{_targetEndpointId}' not active — falling back to default");
                pinned?.Dispose();
            }
            catch (Exception ex)
            {
                _log?.Warn("WindowsAudio", $"Pinned endpoint lookup failed: {ex.GetType().Name}: {ex.Message} — falling back to default");
            }
        }

        _device = en.GetDefaultAudioEndpoint(DataFlow.Render, Role.Multimedia);
        _volume = _device.AudioEndpointVolume;
        _volume.OnVolumeNotification += OnNotification;
    }

    /// <summary>Repoint the client to a different render endpoint. Pass null or empty to
    /// go back to following the OS default. Safe to call at any time; performs a locked
    /// detach + attach cycle and emits a fresh snapshot on success.</summary>
    public void SetTargetEndpoint(string? endpointId)
    {
        var next = string.IsNullOrWhiteSpace(endpointId) ? null : endpointId;
        lock (_attachLock)
        {
            if (_targetEndpointId == next) return;
            _targetEndpointId = next;
            if (_enumerator is null) return; // not started yet — Start will honor _targetEndpointId
            try
            {
                DetachFromCurrentDevice();
                AttachToCurrentDefault();
            }
            catch (Exception ex)
            {
                _log?.Error("WindowsAudio", $"SetTargetEndpoint reattach failed: {ex.GetType().Name}: {ex.Message}");
                DetachFromCurrentDevice();
                return;
            }
        }
        EmitSnapshot();
    }

    /// <summary>Enumerates every active render endpoint on the machine. Used by Settings
    /// to populate the endpoint picker. Static because it does not need an attached client.</summary>
    public static IReadOnlyList<WindowsAudioEndpointInfo> EnumerateRenderEndpoints()
    {
        var list = new List<WindowsAudioEndpointInfo>();
        try
        {
            using var en = new MMDeviceEnumerator();
            var devs = en.EnumerateAudioEndPoints(DataFlow.Render, DeviceState.Active);
            foreach (var d in devs)
            {
                try { list.Add(new WindowsAudioEndpointInfo(d.ID, ShortenFriendlyName(d.FriendlyName ?? "Unknown device"))); }
                finally { d.Dispose(); }
            }
        }
        catch
        {
            // Headless / broken audio stack — return whatever we managed to collect.
        }
        return list;
    }

    /// <summary>Strips the parenthesized adapter suffix Windows appends to endpoint names,
    /// e.g. "SteelSeries Sonar - Chat (SteelSeries Sonar Virtual Audio Device)" →
    /// "SteelSeries Sonar - Chat". Keeps single-adapter endpoints intact
    /// (e.g. "Hoparlör (Realtek(R) Audio)" → "Hoparlör (Realtek)") so identical-named
    /// endpoints on different drivers stay distinguishable. Used only for display —
    /// the raw endpoint id is what's persisted and matched.</summary>
    private static string ShortenFriendlyName(string full)
    {
        int paren = full.LastIndexOf(" (", StringComparison.Ordinal);
        if (paren <= 0) return full;
        var head = full.Substring(0, paren);
        var tail = full.Substring(paren + 2, full.Length - paren - 3); // strip " (" and trailing ")"
        // If the head already contains the adapter descriptor (Sonar Chat / Sonar Gaming /
        // Sonar Media all bundle the driver name in parens), drop the tail entirely.
        if (head.Contains(tail, StringComparison.OrdinalIgnoreCase)) return head;
        // Otherwise keep a shortened adapter hint so duplicates stay tellable.
        // Pull the first two words out of the adapter descriptor.
        var words = tail.Split(' ', StringSplitOptions.RemoveEmptyEntries);
        var shortAdapter = words.Length switch
        {
            0 => tail,
            1 => words[0],
            _ => words[0] + " " + words[1],
        };
        return $"{head} ({shortAdapter})";
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
        // If the user pinned a specific endpoint, changes to the OS default don't concern us —
        // Sonar users pinning "Chat" don't want the OSD to jump when Windows re-picks default
        // Speakers on a headset unplug.
        if (!string.IsNullOrEmpty(_targetEndpointId))
        {
            _log?.Info("WindowsAudio", $"OnDefaultDeviceChanged ignored (pinned to {_targetEndpointId})");
            return;
        }
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

    public void OnDeviceStateChanged(string deviceId, DeviceState newState)
    {
        // When our pinned endpoint disappears (Sonar restart, device unplug), the current
        // device handle is stale — reattach so the picker falls back to default silently.
        // Also handles the reverse: pinned endpoint comes back active → resume it.
        if (string.IsNullOrEmpty(_targetEndpointId)) return;
        if (!string.Equals(deviceId, _targetEndpointId, StringComparison.OrdinalIgnoreCase)) return;

        _log?.Info("WindowsAudio", $"OnDeviceStateChanged: pinned endpoint {deviceId} -> {newState}");
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
                DetachFromCurrentDevice();
                return;
            }
        }
        EmitSnapshot();
    }
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
