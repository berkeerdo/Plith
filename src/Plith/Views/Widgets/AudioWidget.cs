using System.Windows;
using System.Windows.Controls;
using Plith.Services;
using Plith.ViewModels;

namespace Plith.Views.Widgets;

/// <summary>
/// The level, the rail it belongs to, and a track you can actually drag.
///
/// This is the widget only Plith can offer: the app already knows every rail — the endpoint, the
/// Voicemeeter bus, the level — and no other notch shell has an audio engine underneath it.
///
/// The interesting problem here is not drawing the track. It is that the same value arrives from
/// two directions: the user drags it, and the endpoint or the parameter poll reports it. Writing
/// the display on both routes makes a drag fight the poll for the same pixel, which reads as a
/// slider that stutters and snaps backwards under the finger. So the rule is one-directional at
/// any moment: while the user is driving the track, incoming reports are ignored; the rest of
/// the time the track follows them exactly.
/// </summary>
public partial class AudioWidget : UserControl
{
    private readonly AudioCardViewModel _vm;
    private readonly Func<double, bool> _write;

    /// <summary>
    /// When the user last moved the track themselves.
    ///
    /// A timestamp rather than a flag, and that is a correction rather than a preference. The
    /// flag version was set on every user-driven change and cleared on mouse-up, lost capture
    /// and lost focus — which covers a drag and a click, and does NOT cover a keyboard arrow. A
    /// person who tabbed to the track and pressed Left once left it set for as long as focus
    /// stayed there, and every incoming report was ignored for that whole time: the display
    /// silently stopped following the volume keys.
    ///
    /// A window that expires cannot get stuck. It also covers the routes the flag covered, for
    /// the same reason — every one of them lands in OnValueChanged.
    /// </summary>
    private long _lastUserChangeMs = long.MinValue;

    /// <summary>
    /// How long after a user-driven change incoming reports stay suppressed.
    ///
    /// Long enough to cover the gap between a drag's ticks and the report they cause coming back
    /// through the poll — the parameter poll runs on its own cadence and the echo of our own
    /// write is what would otherwise pull the thumb back under the finger. Short enough that a
    /// track left focused starts following external changes again almost immediately.
    /// </summary>
    private const long UserDrivingWindowMs = 350;

    private bool UserIsDriving => Environment.TickCount64 - _lastUserChangeMs < UserDrivingWindowMs;

    /// <summary>The last value written to the track from a report, so an echo of our own write
    /// coming back through the poll is recognised rather than treated as a fresh report that
    /// needs pushing into the control again.</summary>
    private double _lastReported = double.NaN;

    public AudioWidget(AudioCardViewModel vm, Func<double, bool> write)
    {
        ArgumentNullException.ThrowIfNull(vm);
        ArgumentNullException.ThrowIfNull(write);

        InitializeComponent();
        _vm = vm;
        _write = write;

        // Not a binding. A two-way binding on a value that also arrives from outside is exactly
        // the fight described above: the binding would write back on every incoming report, and
        // the report would arrive again as a change. The two directions are kept explicit so
        // each one has a place to be suppressed.
        _vm.PropertyChanged += OnViewModelChanged;

        Track.ValueChanged += OnTrackValueChanged;

        IsVisibleChanged += (_, e) => { if ((bool)e.NewValue) Render(); };
        Render();
    }

    private void OnViewModelChanged(object? sender, System.ComponentModel.PropertyChangedEventArgs e) => Render();

    /// <summary>
    /// Paint from the view model.
    ///
    /// Skipped entirely while the user is driving the track. That is the one rule that keeps a
    /// drag smooth: a report arriving mid-gesture describes the level a moment ago, and writing
    /// it to the control pulls the thumb back out from under the finger.
    /// </summary>
    private void Render()
    {
        Source.Text = _vm.Label;
        Level.Text = _vm.Muted ? "Muted" : _vm.GainText;

        if (UserIsDriving) return;

        var reported = VolumeMath.Clamp01(_vm.GainNormalized);
        if (reported.Equals(_lastReported)) return;

        _lastReported = reported;

        // Detach while writing, so this write does not come straight back through
        // OnTrackValueChanged and turn a report into a write to the audio device.
        Track.ValueChanged -= OnTrackValueChanged;
        Track.Value = reported;
        Track.ValueChanged += OnTrackValueChanged;

        UpdateAnnouncedValue();
    }

    private void OnTrackValueChanged(object sender, RoutedPropertyChangedEventArgs<double> e)
    {
        _lastUserChangeMs = Environment.TickCount64;

        // Snapped, so a drag lands on a round number rather than 47.318 %. The snap happens
        // before the write, not after: snapping the display alone would show a value the device
        // was never set to.
        var snapped = VolumeMath.SnapToStep(e.NewValue, StepPercent);

        if (!_write(snapped))
        {
            // Nothing accepted the write - no Voicemeeter, no attached endpoint. The control is
            // left where the user put it rather than snapping to a value nothing holds, and the
            // next report will correct it if one ever comes.
            return;
        }

        // The display is NOT written here. It comes back through the endpoint notification or
        // the parameter poll, on the same route every other change takes. This is the whole
        // reason the write path does not echo either — see OsdOrchestrator.TrySetNormalizedVolume.
        UpdateAnnouncedValue();
    }

    /// <summary>Drag granularity, in per cent. Two per cent is fine enough that the track never
    /// feels notched under the finger and coarse enough that the number reads as deliberate.</summary>
    private const double StepPercent = 2;

    /// <summary>
    /// Keep the announced value in step with the drawn one.
    ///
    /// A Slider's automation peer reports its raw 0..1, which a screen reader reads out as
    /// "0.62" — true, and useless next to a display that says 62 %. The announced text says what
    /// the widget says.
    /// </summary>
    private void UpdateAnnouncedValue() =>
        System.Windows.Automation.AutomationProperties.SetHelpText(Track, Level.Text);
}
