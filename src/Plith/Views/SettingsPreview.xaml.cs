using System.ComponentModel;
using System.Windows.Controls;
using Plith.Services;
using Plith.ViewModels;
using Plith.Views.Presentation;

namespace Plith.Views;

/// <summary>
/// Mini OSD card rendered next to the settings list. Updates live as the user drags sliders /
/// flips toggles in the settings window.
///
/// Deliberately a hand-built mock: it holds its own card view models seeded with sample text
/// and never acquires a <see cref="Plith.Cards.CardHost"/>, because a second show pipeline
/// inside the Settings window would be a second OSD authority in the app.
/// </summary>
public partial class SettingsPreview : UserControl, INotifyPropertyChanged
{
    public AudioCardViewModel PreviewAudio { get; } = new()
    {
        Label = "Bus A1",
        GainText = "+3.0 dB",
        GainNormalized = 0.85,
    };

    public MediaViewModel PreviewMedia { get; } = new()
    {
        Title = "Sample track",
        Artist = "Sample artist",
        HasSession = true,
    };

    private bool _showMediaCard = true;

    /// <summary>Mirrors what CompactMode does to the real OSD's media card. Local state
    /// rather than a MediaCard, because the preview has no settings-driven card pipeline.</summary>
    public bool ShowMediaCard
    {
        get => _showMediaCard;
        private set
        {
            if (_showMediaCard == value) return;
            _showMediaCard = value;
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(nameof(ShowMediaCard)));
        }
    }

    public event PropertyChangedEventHandler? PropertyChanged;

    public SettingsPreview()
    {
        DataContext = this;
        InitializeComponent();

        // Re-anchor the preview card whenever the surface resizes, otherwise the BottomCenter
        // anchor stops looking like "bottom centre" if the user resizes the settings window.
        PreviewSurface.SizeChanged += (_, _) =>
        {
            UpdatePosition(_lastPosition);
            if (MiniNotch.Visibility == System.Windows.Visibility.Visible) ApplyNotchShape(_lastRestingHeight);
        };
    }

    private OsdPosition _lastPosition = OsdPosition.BottomCenter;

    public void UpdatePosition(OsdPosition position)
    {
        _lastPosition = position;
        MiniCard.HorizontalAlignment = position switch
        {
            OsdPosition.BottomRight or OsdPosition.TopRight => System.Windows.HorizontalAlignment.Right,
            _ => System.Windows.HorizontalAlignment.Center,
        };
        MiniCard.VerticalAlignment = position switch
        {
            OsdPosition.TopCenter or OsdPosition.TopRight => System.Windows.VerticalAlignment.Top,
            _ => System.Windows.VerticalAlignment.Bottom,
        };
    }

    /// <summary>
    /// Switch the preview between the two presentations, and set the notch's resting height.
    ///
    /// Both in one call because they are one question — what the OSD looks like when nothing is
    /// happening — and splitting them would let the preview show a resting height for a
    /// presentation that does not have one.
    ///
    /// The classic card is hidden entirely in notch mode rather than dimmed: at rest the notch
    /// IS the whole OSD, and leaving a card floating beside it would show a state the user can
    /// never reach.
    /// </summary>
    public void UpdatePresentation(PresentationMode mode, double restingHeightDip)
    {
        var notch = mode == PresentationMode.AmbientNotch;

        MiniNotch.Visibility = notch ? System.Windows.Visibility.Visible : System.Windows.Visibility.Collapsed;
        MiniCard.Visibility = notch ? System.Windows.Visibility.Collapsed : System.Windows.Visibility.Visible;
        if (notch) ApplyNotchShape(restingHeightDip);
    }

    /// <summary>
    /// Size the drawn notch to the preview surface.
    ///
    /// Scaled by the surface's width against a real screen's, so the shape keeps its true
    /// proportion instead of being a fixed rectangle that happens to look about right. Re-run
    /// on every size change, because the settings window is resizable and a scale computed once
    /// would be wrong the moment it is.
    /// </summary>
    private void ApplyNotchShape(double restingHeightDip)
    {
        _lastRestingHeight = restingHeightDip;

        var surfaceWidth = PreviewSurface.ActualWidth;
        if (surfaceWidth <= 0) return;

        // 1920 is a stand-in for "a screen", not a claim about the user's. It only sets how
        // small the notch looks relative to the preview, and being out by a monitor size changes
        // nothing a person would read from this.
        const double AssumedScreenWidthDip = 1920;
        var scale = surfaceWidth / AssumedScreenWidthDip;

        MiniNotch.Width = Math.Max(2, NotchGeometry.CollapsedWidthDip * scale);
        MiniNotch.Height = Math.Max(1, restingHeightDip * scale * NotchHeightExaggeration);

        var radius = Math.Min(NotchGeometry.CollapsedRadiusDip * scale, MiniNotch.Height);
        MiniNotch.CornerRadius = new System.Windows.CornerRadius(0, 0, radius, radius);
    }

    /// <summary>
    /// The resting height is drawn taller than true scale.
    ///
    /// At true scale a 5 DIP notch on a preview a tenth of a screen wide is half a pixel, and
    /// the slider would appear to do nothing at all. The setting's whole purpose is choosing
    /// between values in that range, so a preview that cannot show the difference is worse than
    /// no preview. Deliberately exaggerated, and only on this axis.
    /// </summary>
    private const double NotchHeightExaggeration = 6;

    private double _lastRestingHeight = 5;

    public void UpdateOpacity(double percent01) => MiniCard.Opacity = percent01;

    public void UpdateCompact(bool compact) => ShowMediaCard = !compact;

    public void UpdateColorThresholds(bool thresholds) => PreviewAudio.UseColorThresholds = thresholds;

}
