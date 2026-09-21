using System.Windows;
using System.Windows.Automation;
using System.Windows.Controls;
using Plith.Services;
using Plith.ViewModels;
using Plith.Views.Presentation;

namespace Plith.Views.Widgets;

/// <summary>Which answer the HUD is giving.</summary>
public enum NotchHudKind
{
    Volume,
    Media,
    Brightness,
}

/// <summary>
/// The notch's answer to something the user just did: a volume key, a track change.
///
/// A separate shape family from the widget frame, and the separation is the design rather than a
/// detail. The frame is a place you went and is held while you use it; this is short, wide, and
/// gone in two seconds. Before this, an event opened the same panel a click did, which is why
/// the OSD read as a window rather than as feedback.
///
/// One thing here is interactive, and exactly one: the speaker mutes. That is a deliberate
/// narrowing rather than a relaxation of the old rule. The volume page used to carry the mute
/// and the draggable level, and it was removed because a page you reach by swiping is the wrong
/// place for the answer to a key you just pressed — which left mute with nowhere to live. One
/// hit target on the surface where volume already is beats a page nobody swipes to.
///
/// Everything else stays inert: no transport to press by accident while reaching for a browser
/// tab underneath, and the shape is still gone in two seconds.
/// </summary>
public partial class NotchHud : UserControl
{
    private readonly AudioCardViewModel _audio;
    private readonly MediaViewModel _media;
    private readonly BrightnessCardViewModel? _brightness;

    /// <param name="toggleMute">Optional. Null leaves the speaker inert rather than pretending
    /// to be a control — a button that does nothing when pressed teaches people not to trust the
    /// ones that do.</param>
    /// <param name="brightness">Optional. Null leaves the brightness kind unreachable rather
    /// than drawing an empty bar, which is the right answer on a build with no brightness
    /// source wired.</param>
    public NotchHud(AudioCardViewModel audio, MediaViewModel media, Func<bool>? toggleMute = null,
                    BrightnessCardViewModel? brightness = null)
    {
        ArgumentNullException.ThrowIfNull(audio);
        ArgumentNullException.ThrowIfNull(media);

        InitializeComponent();
        _audio = audio;
        _media = media;
        _brightness = brightness;

        if (toggleMute is null) MuteButton.IsEnabled = false;
        else MuteButton.Click += (_, _) => toggleMute();

        // Live while showing, because a volume key held down produces a stream of changes and a
        // HUD that painted once would sit there showing the first of them.
        _audio.PropertyChanged += (_, _) => { if (Kind == NotchHudKind.Volume) RenderVolume(); };
        _media.PropertyChanged += (_, _) => { if (Kind == NotchHudKind.Media) RenderMedia(); };
        if (_brightness is not null)
            _brightness.PropertyChanged += (_, _) => { if (Kind == NotchHudKind.Brightness) RenderBrightness(); };

        Show(NotchHudKind.Volume);
    }

    public NotchHudKind Kind { get; private set; } = NotchHudKind.Volume;

    /// <summary>
    /// Switch which answer is showing, and with it the HUD's size.
    ///
    /// The size is set on this control, and the notch's surface follows the measured content, so
    /// there is exactly one place that decides how wide a media HUD is. Painting happens here
    /// too rather than only on the next notification: an event is the first thing the user sees,
    /// and a HUD that waited for a change would show the previous one for a frame.
    /// </summary>
    public void Show(NotchHudKind kind)
    {
        Kind = kind;

        // Brightness takes the narrow shape, like volume: it is a level and a number, and the
        // wide one exists for a title that needs the room.
        var size = kind == NotchHudKind.Media ? NotchGeometry.HudWideDip : NotchGeometry.HudDip;
        Width = size.Width;
        Height = size.Height;

        VolumeRow.Visibility = kind == NotchHudKind.Volume ? Visibility.Visible : Visibility.Collapsed;
        MediaRow.Visibility = kind == NotchHudKind.Media ? Visibility.Visible : Visibility.Collapsed;
        BrightnessRow.Visibility = kind == NotchHudKind.Brightness ? Visibility.Visible : Visibility.Collapsed;

        switch (kind)
        {
            case NotchHudKind.Media: RenderMedia(); break;
            case NotchHudKind.Brightness: RenderBrightness(); break;
            default: RenderVolume(); break;
        }
    }

    private void RenderVolume()
    {
        LevelText.Text = _audio.Muted ? "Muted" : _audio.GainText;

        var t = VolumeMath.Clamp01(_audio.GainNormalized);
        UpdateFill(t);

        // The icon carries the state on its own, so "is my sound off?" is answered without
        // reading anything. Three shapes: crossed when muted, one wave when quiet, two when not.
        MuteCross.Visibility = _audio.Muted ? Visibility.Visible : Visibility.Collapsed;
        WaveInner.Visibility = _audio.Muted ? Visibility.Collapsed : Visibility.Visible;
        WaveOuter.Visibility = _audio.Muted || t < QuietBelow ? Visibility.Collapsed : Visibility.Visible;

        LevelFill.Background = _audio.GainColor;

        // ON THIS CONTROL, not on the row. The comment here used to say "on the row, which has a
        // peer" and that was simply false: WPF gives a Grid no automation peer, so the name
        // reached nothing and a screen reader following the OSD heard silence. The scan that says
        // so had two of these three rows on a known-gap list rather than fixed, and the third
        // arrived with brightness and failed the build, which is what got all three looked at.
        //
        // The root is the right place and not merely an available one: exactly one row is visible
        // at a time (see Show), so the HUD has one reading, and it is whichever row is up.
        AutomationProperties.SetName(this, _audio.AccessibleSummary);
    }

    /// <summary>Below this the speaker drops its outer wave. A third of the way is where "quiet"
    /// stops being a guess and starts matching what people call it.</summary>
    private const double QuietBelow = 0.33;

    /// <summary>
    /// Size the fill from the track's actual width.
    ///
    /// Read at the point of use, never remembered: the HUD changes width between its two kinds,
    /// and a fill sized from a width captured earlier would be drawn against the other one. If
    /// the track has not been arranged yet the fill is left alone rather than being set to zero,
    /// because a bar that flashes empty on every event is worse than one that arrives a frame
    /// late.
    /// </summary>
    private void UpdateFill(double t) => UpdateFill(LevelFill, t);

    private static void UpdateFill(Border fill, double t)
    {
        if (fill.Parent is not FrameworkElement track) return;
        var available = track.ActualWidth;
        if (available <= 0) return;

        fill.Width = available * t;
    }

    private void RenderBrightness()
    {
        if (_brightness is null) return;

        BrightnessText.Text = _brightness.DisplayText;
        UpdateFill(BrightnessFill, _brightness.Normalized);

        // On this control. See RenderVolume for why the row cannot carry it.
        AutomationProperties.SetName(this, _brightness.AccessibleSummary);
    }

    private void RenderMedia()
    {
        Title.Text = _media.HasSession ? _media.Title : "Nothing playing";
        Artist.Text = _media.HasSession ? _media.Artist : string.Empty;
        Art.Source = _media.AlbumArt;

        // On this control. See RenderVolume for why the row cannot carry it.
        AutomationProperties.SetName(this, _media.AccessibleSummary);
    }

    protected override Size ArrangeOverride(Size arrangeBounds)
    {
        var result = base.ArrangeOverride(arrangeBounds);

        // The fill depends on a width that only exists after arrange, so it is recomputed here
        // as well as on every change. Doing it only on change would leave the bar unpainted the
        // first time a HUD of this width is shown - the exact class of defect that comes from
        // reading a measurement before the tree has one.
        if (Kind == NotchHudKind.Volume) UpdateFill(VolumeMath.Clamp01(_audio.GainNormalized));
        else if (Kind == NotchHudKind.Brightness && _brightness is not null) UpdateFill(BrightnessFill, _brightness.Normalized);

        return result;
    }
}
