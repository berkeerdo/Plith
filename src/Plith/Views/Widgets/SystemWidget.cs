using System.Globalization;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
using Plith.Services;

namespace Plith.Views.Widgets;

/// <summary>
/// The controls that belong to the machine rather than to what is playing on it: display
/// brightness, and whether the microphone is listening.
///
/// The roadmap called this a System Controls *card*, and it is a page instead. That is not a
/// rename. Cards were the shape the OSD had when everything it showed was an answer to something
/// you just pressed; the notch replaced that with two distinct shapes — a HUD that answers a key,
/// and a frame you deliberately open. Nobody presses a key to be told their microphone's state,
/// so a card would have been a thing that appeared without ever having been asked for.
///
/// Both halves collapse independently, and the page is only installed when at least one of them
/// answered — see <see cref="HasAnything"/>. A brightness rail on a machine whose display refuses
/// DDC/CI, or a mic button on a desktop with no capture device, would be present and dead, which
/// is worse than absent.
/// </summary>
public partial class SystemWidget : UserControl
{
    private readonly BrightnessClient? _brightness;
    private readonly Func<MicrophoneSnapshot?>? _microphone;
    private readonly Func<bool?>? _toggleMicMute;

    /// <summary>
    /// When the user last moved the rail themselves.
    ///
    /// The same guard the audio rail carries, and it exists here for a sharper reason. Brightness
    /// arrives from two directions too — the finger, and <see cref="BrightnessClient.Changed"/>
    /// confirming what landed — but the confirmation comes back over a cable that takes tens of
    /// milliseconds at best. Writing the display on both routes means the thumb is repeatedly
    /// yanked back to where the monitor has got to, several positions behind the finger.
    ///
    /// A timestamp rather than a flag, for the reason recorded on the audio rail: the flag
    /// version cleared on mouse-up and lost focus, which covers a drag and misses a keyboard
    /// arrow entirely.
    /// </summary>
    private long _lastUserChangeMs;
    private bool _hasUserChanged;

    /// <summary>
    /// How long a user-driven change suppresses incoming reports.
    ///
    /// Longer than the audio rail's 350 ms, and deliberately: that window covers the echo of a
    /// write coming back through an in-process poll, while this one has to cover a round trip
    /// down a display cable. Too short and the confirmation of an earlier position arrives while
    /// the finger is already elsewhere.
    /// </summary>
    private const long UserDrivingWindowMs = 900;

    private bool UserIsDriving => _hasUserChanged && Environment.TickCount64 - _lastUserChangeMs < UserDrivingWindowMs;

    /// <summary>Whether this page has anything at all to show. False means it must not be
    /// installed: an empty page is still a page you can swipe to and be told nothing on.</summary>
    public bool HasAnything => _brightness?.Current is not null || _microphone?.Invoke() is not null;

    /// <param name="brightness">Optional, and optional in practice as well as in signature — a
    /// display that refuses DDC/CI leaves Current null and collapses the whole rail.</param>
    /// <param name="microphone">Optional. Null, or a machine with no capture device, removes the
    /// button rather than showing an unmuted microphone nobody has.</param>
    /// <param name="toggleMicMute">Optional. Present without it, the button would be a state that
    /// looks pressable and is not, so its absence hides the button too.</param>
    public SystemWidget(BrightnessClient? brightness = null,
                        Func<MicrophoneSnapshot?>? microphone = null,
                        Func<bool?>? toggleMicMute = null)
    {
        InitializeComponent();
        _brightness = brightness;
        _microphone = microphone;
        _toggleMicMute = toggleMicMute;

        Track.ValueChanged += OnTrackChanged;

        if (_brightness is not null)
        {
            // Not marshalled by the client - the contract is that it raises on its writer thread
            // and every subscriber hops its own dispatcher.
            _brightness.Changed += level => Dispatcher.BeginInvoke(() => OnReported(level));
        }

        IsVisibleChanged += (_, e) =>
        {
            if (!(bool)e.NewValue) return;

            // The accent is resolved here rather than only once: it lives in the palette merged
            // at the OSD window, and it changes when Appearance changes. Every open re-resolves,
            // which is cheap and means the rail cannot be left wearing the previous accent.
            ApplyAccent();

            // Re-read on open. The monitor's own buttons change brightness with no notification
            // of any kind, so a value from the last time this page was shown can be arbitrarily
            // stale. Off the UI thread: a DDC/CI read is far too slow to sit inside a paint.
            RefreshBrightnessAsync();

            Render();
        };

        // Painted once at construction as well, for the reason the other pages carry it: bound to
        // IsVisibleChanged alone, a page that has never been shown holds nothing - true offscreen,
        // and true for any host that lays it out before it is on a rendered surface.
        Render();
    }

    /// <summary>Repaint from outside, when something the page reads has changed but the page has
    /// no way of knowing. The mic's own notification is what calls this.</summary>
    public void Refresh() => Render();

    private void ApplyAccent()
    {
        if (TryFindResource("OsdAccent") is Brush accent) Resources["NotchTrackFill"] = accent;
    }

    private void RefreshBrightnessAsync()
    {
        var client = _brightness;
        if (client?.Current is null) return;

        // A read that lands while the finger is on the rail is dropped rather than applied, the
        // same way a report is - it would be a value from before the drag started.
        Task.Run(() =>
        {
            var level = client.Refresh();
            if (level is { } value) Dispatcher.BeginInvoke(() => OnReported(value));
        });
    }

    private void OnReported(int level)
    {
        if (UserIsDriving) return;

        _suppressWrite = true;
        Track.Value = level;
        _suppressWrite = false;

        RenderBrightness(level);
    }

    /// <summary>Set while the code is writing the track, so the resulting ValueChanged is not
    /// mistaken for the user moving it — which would start the driving window and make every
    /// incoming report suppress the next one.</summary>
    private bool _suppressWrite;

    private void OnTrackChanged(object sender, RoutedPropertyChangedEventArgs<double> e)
    {
        var level = (int)Math.Round(e.NewValue);

        // Painted first, always. The display may take most of a second to get there and may
        // refuse outright; the number under the finger has to move now either way, and the
        // client corrects it if the display disagrees.
        RenderBrightness(level);

        if (_suppressWrite) return;

        _hasUserChanged = true;
        _lastUserChangeMs = Environment.TickCount64;
        _brightness?.TrySet(level);
    }

    private void OnMicClick(object sender, RoutedEventArgs e)
    {
        _toggleMicMute?.Invoke();

        // Repainted from the reading rather than from what the toggle returned. The endpoint is
        // the authority on its own mute, and its notification will arrive anyway - painting the
        // returned value would briefly show our intention instead of the state.
        Render();
    }

    private void Render()
    {
        // Also here, not only on becoming visible. A host that lays the control out without ever
        // making it visible - the offscreen render harness is exactly that - would otherwise
        // photograph the rail wearing its placeholder colour and call it the design.
        ApplyAccent();

        RenderMic();

        var level = _brightness?.Current;
        var hasBrightness = level is not null;

        BrightnessBlock.Visibility = hasBrightness ? Visibility.Visible : Visibility.Collapsed;
        Track.Visibility = hasBrightness ? Visibility.Visible : Visibility.Collapsed;

        if (level is { } value)
        {
            if (!UserIsDriving)
            {
                _suppressWrite = true;
                Track.Value = value;
                _suppressWrite = false;
            }

            RenderBrightness((int)Math.Round(Track.Value));
        }

    }

    private void RenderBrightness(int level)
    {
        Level.Text = string.Create(CultureInfo.CurrentCulture, $"{level}%");

        // The announced name carries what the number means. "40%" alone, read out on its own,
        // says nothing about what is at forty percent.
        System.Windows.Automation.AutomationProperties.SetName(
            Level, $"Display brightness {level} percent");
    }

    private void RenderMic()
    {
        var mic = _microphone?.Invoke();

        // Hidden without a way to act on it as well as without a device. A button that shows a
        // state and does nothing when pressed is a worse answer than no button.
        var usable = mic is not null && _toggleMicMute is not null;
        MicButton.Visibility = usable ? Visibility.Visible : Visibility.Collapsed;
        if (mic is not { } reading) return;

        MicCross.Visibility = reading.Muted ? Visibility.Visible : Visibility.Collapsed;

        // Muted is drawn muted, not merely crossed out. The cross alone reads as decoration at
        // this size; dropping the whole glyph to the muted ink is what makes the state legible
        // from a glance rather than from a look.
        var ink = (reading.Muted ? TryFindResource("NotchInkMuted") : TryFindResource("NotchInk")) as Brush;
        if (ink is not null)
        {
            MicBody.Fill = ink;
            MicStand.Stroke = ink;
            MicCross.Stroke = ink;
        }

        // Replaces the static "Microphone" the XAML carries. That placeholder is not redundant:
        // the accessibility lint reads XAML, not running controls, and a name that only ever
        // exists at runtime is a name the gate cannot see — which is how three of the four
        // defects in the "completed" accessibility pass got through.
        System.Windows.Automation.AutomationProperties.SetName(
            MicButton, reading.Muted
                ? $"Microphone muted, {reading.DeviceLabel}. Activate to unmute."
                : $"Microphone on, {reading.DeviceLabel}. Activate to mute.");
    }

}
