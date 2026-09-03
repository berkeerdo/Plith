using System.ComponentModel;
using System.Windows.Controls;
using Plith.Services;
using Plith.ViewModels;

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
        PreviewSurface.SizeChanged += (_, _) => UpdatePosition(_lastPosition);
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

    public void UpdateOpacity(double percent01) => MiniCard.Opacity = percent01;

    public void UpdateCompact(bool compact) => ShowMediaCard = !compact;

    public void UpdateColorThresholds(bool thresholds) => PreviewAudio.UseColorThresholds = thresholds;

}
