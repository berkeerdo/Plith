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

    /// <summary>
    /// Set the attached endpoint's master volume, 0..1.
    ///
    /// Returns false when nothing is attached rather than throwing: an endpoint can be pulled
    /// out from under a drag that is already in flight, and a headset unplugged mid-gesture is
    /// an ordinary event, not an error.
    ///
    /// The value is read back by the endpoint's own volume notification, on its own schedule.
    /// Nothing is echoed into the view model here — see TrySetGain in VoicemeeterClient for why
    /// giving one source two routes into the UI is what makes a drag fight the poll.
    ///
    /// Takes the attach lock for the same reason every other reader does: Stop() can null
    /// _volume between the check and the write, and a COM call on a disposed endpoint is not a
    /// recoverable failure.
    /// </summary>
    /// <summary>
    /// Flip the attached endpoint's mute.
    ///
    /// Returns the new state, or null when there is nothing attached — a caller cannot tell
    /// "muted" from "failed" if both come back as false.
    /// </summary>
    public bool? TryToggleMute()
    {
        lock (_attachLock)
        {
            var volume = _volume;
            if (volume is null) return null;

            try
            {
                var next = !volume.Mute;
                volume.Mute = next;
                return next;
            }
            catch (System.Runtime.InteropServices.COMException ex)
            {
                _log?.Warn("WindowsAudioClient", $"Mute toggle failed: {ExceptionText.Describe(ex)}");
                return null;
            }
        }
    }

    public bool TrySetScalarVolume(float scalar)
    {
        lock (_attachLock)
        {
            var volume = _volume;
            if (volume is null) return false;

            try
            {
                volume.MasterVolumeLevelScalar = Math.Clamp(scalar, 0f, 1f);
                return true;
            }
            catch (System.Runtime.InteropServices.COMException ex)
            {
                // The endpoint went away between the lock and the write. Logged rather than
                // swallowed, because a write path that fails silently is indistinguishable from
                // a slider that does not work.
                _log?.Warn("WindowsAudioClient", $"Volume write failed: {ExceptionText.Describe(ex)}");
                return false;
            }
        }
    }

    public bool Start()
    {
        MMDeviceEnumerator? enToDrain = null;
        bool ok = false;
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
                ok = true;
            }
            catch (Exception ex)
            {
                _log?.Error("WindowsAudio", $"Start failed: {ex.GetType().Name}: {ex.Message}");
                // No audio endpoint (headless box, broken driver). Caller will fall back or retry.
                // Detach happens under the lock; enumerator drain must be outside the lock
                // (see the note on Stop() for the deadlock this avoids).
                DetachFromCurrentDevice();
                enToDrain = Interlocked.Exchange(ref _enumerator, null);
            }
        }
        if (enToDrain is not null)
        {
            try { enToDrain.UnregisterEndpointNotificationCallback(this); } catch { }
            enToDrain.Dispose();
        }
        if (ok) EmitSnapshot();
        return ok;
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

        // Logged for the same reason the failure now is. The pinned branch above says which
        // endpoint it took; this one said nothing, so after a default-device change the log went
        // quiet whether the re-attach had worked or not.
        _log?.Info("WindowsAudio", $"Attached to default endpoint '{AudioLabel.Shorten(_device.FriendlyName)}'");
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
        // Ids and names are collected first and shortened as a SET afterwards. Shortening each
        // name as it arrived produced two identical labels on this machine, because
        // AudioLabel.Shorten keeps the adapter's first two words and two Steam devices share
        // them. See AudioLabel.ShortenAll.
        var ids = new List<string>();
        var names = new List<string>();
        try
        {
            using var en = new MMDeviceEnumerator();
            var devs = en.EnumerateAudioEndPoints(DataFlow.Render, DeviceState.Active);
            foreach (var d in devs)
            {
                try
                {
                    // BOTH read before EITHER is added. The two lists are paired by index
                    // downstream, so a device whose id reads and whose name throws would shift
                    // every label after it by one and put a plausible name on the wrong output.
                    var id = d.ID;
                    var name = d.FriendlyName;
                    ids.Add(id);
                    names.Add(name);
                }
                finally { d.Dispose(); }
            }
        }
        catch
        {
            // Headless / broken audio stack — return whatever we managed to collect.
        }

        var labels = AudioLabel.ShortenAll(names);
        var list = new List<WindowsAudioEndpointInfo>(ids.Count);
        for (var i = 0; i < ids.Count; i++) list.Add(new WindowsAudioEndpointInfo(ids[i], labels[i]));
        return list;
    }

    /// <summary>
    /// The id of the current default render endpoint, or null when it cannot be read.
    ///
    /// Beside the enumeration because the output picker needs both and nothing else exposes the
    /// default's id. Same contract as its neighbour: a broken audio stack costs the caller a
    /// null rather than an exception.
    /// </summary>
    public static string? TryGetDefaultRenderEndpointId()
    {
        try
        {
            using var en = new MMDeviceEnumerator();
            using var def = en.GetDefaultAudioEndpoint(DataFlow.Render, Role.Multimedia);
            return def.ID;
        }
        catch
        {
            return null;
        }
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
        MMDeviceEnumerator? en;
        lock (_attachLock)
        {
            DetachFromCurrentDevice();
            en = Interlocked.Exchange(ref _enumerator, null);
        }
        // Drain the enumerator OUTSIDE the lock. UnregisterEndpointNotificationCallback
        // synchronously waits for any in-flight MTA callback (OnDefaultDeviceChanged /
        // OnDeviceStateChanged) to return before it completes. Those callbacks acquire
        // _attachLock at entry; if we held the lock across the unregister call, the
        // callback would wait on the lock, the unregister would wait on the callback,
        // and the UI thread would deadlock — freezing shutdown and (via WH_KEYBOARD_LL
        // starvation) making the whole system feel unresponsive. Releasing the lock and
        // clearing _enumerator to null lets the callback take the lock, see the null,
        // and exit fast so the unregister can complete.
        if (en is not null)
        {
            try { en.UnregisterEndpointNotificationCallback(this); } catch { }
            en.Dispose();
        }
    }

    /// <summary>When a volume notification was last written to the log, so a sweep of the volume
    /// wheel produces one line rather than forty.</summary>
    private long _lastNotificationLogMs;

    private const long NotificationLogThrottleMs = 2000;

    private void OnNotification(AudioVolumeNotificationData data)
    {
        // Throttled, and kept rather than removed after the investigation it was added for.
        // The failure it exists to show is the absence of these lines: an OSD frozen on a stale
        // level looks exactly like one whose endpoint stopped reporting, and from the outside
        // those are indistinguishable. One line every two seconds is the difference between
        // "the endpoint is silent" and "the endpoint is fine and the display is wrong".
        var now = Environment.TickCount64;
        if (now - _lastNotificationLogMs >= NotificationLogThrottleMs)
        {
            _lastNotificationLogMs = now;
            _log?.Info("WindowsAudio",
                $"Volume notification: {data.MasterVolume * 100:0}%{(data.Muted ? " (muted)" : string.Empty)}");
        }

        EmitSnapshot();
    }

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
            // Shortened here, not only in the endpoint list. The card took FriendlyName raw
            // while the settings dropdown ran the same string through the shortener, so one
            // endpoint appeared two different ways in two places - and the card's was the long
            // one, ellipsed mid-word.
            label = AudioLabel.Shorten(device.FriendlyName);
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
            catch (Exception ex)
            {
                // Logged, not swallowed. This catch was silent, and the silence cost real time:
                // the OSD froze on a stale level after a default-device change and the log showed
                // the notification arriving and then nothing at all - which looks identical to a
                // successful re-attach. A recovery path that cannot be seen failing is a recovery
                // path nobody can trust.
                _log?.Error("WindowsAudio", $"Re-attach after default-device change failed: {ExceptionText.Describe(ex)}");

                // Drop everything so the orchestrator's reconcile pass can re-try via Start.
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
