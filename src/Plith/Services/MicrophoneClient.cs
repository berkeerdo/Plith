using NAudio.CoreAudioApi;

namespace Plith.Services;

/// <summary>One reading of the default capture endpoint.</summary>
/// <param name="DeviceLabel">Already shortened — see <see cref="AudioLabel.Shorten"/>.</param>
/// <param name="Muted">Whether the endpoint itself is muted. Not whether an app has stopped
/// listening: a call app muting you in its own UI leaves this false, which is exactly the
/// confusion an OSD is well placed to clear up rather than add to.</param>
public sealed record MicrophoneSnapshot(string DeviceLabel, bool Muted, float ScalarVolume);

/// <summary>
/// The microphone, as far as Windows itself is concerned.
///
/// Deliberately a sibling of <see cref="WindowsAudioClient"/> rather than a mode of it. They
/// share an API and almost nothing else: the render endpoint is polled by a volume key and
/// drives the OSD's main card, while this one is asked rarely and answers one question. Folding
/// capture into that class would put two lifetimes, two notification callbacks and two "is it
/// attached" states behind one lock for the sake of avoiding some duplicated ceremony.
///
/// The first piece of the System Controls work, and chosen first for a reason: the roadmap's
/// Full Notch needs a mic indicator before it can exist, and a mute state is the one system
/// control a person changes mid-sentence — which is precisely when a notch is better than a
/// tray menu.
/// </summary>
public sealed class MicrophoneClient : IDisposable
{
    private readonly DiagnosticLog? _log;
    private readonly object _attachLock = new();

    private MMDeviceEnumerator? _enumerator;
    private MMDevice? _device;
    private AudioEndpointVolume? _volume;
    private bool _disposed;

    public MicrophoneClient(DiagnosticLog? log = null) => _log = log;

    /// <summary>Raised when the endpoint reports a change. Not marshalled: the callback arrives
    /// on a COM thread and every subscriber here is expected to hop to its own dispatcher, the
    /// same contract WindowsAudioClient.Changed has.</summary>
    public event Action<MicrophoneSnapshot>? Changed;

    /// <summary>The last reading, or null when nothing is attached. Null means "no microphone",
    /// which is a real state on a desktop and must not be shown as "not muted".</summary>
    public MicrophoneSnapshot? Current { get; private set; }

    public bool Start()
    {
        lock (_attachLock)
        {
            if (_disposed) return false;
            if (_volume is not null) return true;

            try
            {
                _enumerator ??= new MMDeviceEnumerator();
                _device = _enumerator.GetDefaultAudioEndpoint(DataFlow.Capture, Role.Communications);
                _volume = _device.AudioEndpointVolume;
                _volume.OnVolumeNotification += OnNotification;

                _log?.Info("Microphone", $"Attached to '{AudioLabel.Shorten(_device.FriendlyName)}'");
                Emit();
                return true;
            }
            catch (Exception ex)
            {
                // A machine with no capture device throws here, and that is an ordinary state
                // rather than a failure: a desktop without a microphone should show no mic
                // control at all, which Current being null is how it says so.
                _log?.Info("Microphone", $"No capture endpoint: {ExceptionText.Describe(ex)}");
                Detach();
                return false;
            }
        }
    }

    /// <summary>
    /// Flip the microphone's mute. Returns the new state, or null when there is nothing to flip.
    ///
    /// Reads before it writes rather than tracking the state here — another app, or the device's
    /// own hardware switch, can change it at any moment, and a remembered value would flip to
    /// the opposite of what WE last set instead of the opposite of what is true.
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
                _log?.Warn("Microphone", $"Mute toggle failed: {ExceptionText.Describe(ex)}");
                return null;
            }
        }
    }

    private void OnNotification(AudioVolumeNotificationData data) => Emit();

    private void Emit()
    {
        MicrophoneSnapshot? snapshot = null;

        lock (_attachLock)
        {
            var device = _device;
            var volume = _volume;
            if (device is null || volume is null) return;

            try
            {
                snapshot = new MicrophoneSnapshot(
                    AudioLabel.Shorten(device.FriendlyName), volume.Mute, volume.MasterVolumeLevelScalar);
            }
            catch
            {
                // The endpoint went away mid-read. Dropping this pulse is correct: the
                // enumerator's own notification will bring the replacement.
                return;
            }
        }

        Current = snapshot;
        Changed?.Invoke(snapshot!);
    }

    private void Detach()
    {
        if (_volume is not null)
        {
            try { _volume.OnVolumeNotification -= OnNotification; } catch { /* already gone */ }
            _volume = null;
        }

        _device?.Dispose();
        _device = null;
        Current = null;
    }

    public void Dispose()
    {
        lock (_attachLock)
        {
            if (_disposed) return;
            _disposed = true;
            Detach();
            _enumerator?.Dispose();
            _enumerator = null;
        }
    }
}
