using System.Windows.Controls;
using System.Windows.Media;
using System.Windows.Shapes;
using Plith.Services;
using Plith.ViewModels;

namespace Plith.Views;

/// <summary>
/// Mini OSD card rendered next to the settings list. Updates live as the user drags sliders /
/// flips toggles in the settings window. The preview holds its own <see cref="OsdViewModel"/>
/// seeded with sample text so the user sees realistic placeholder content.
/// </summary>
public partial class SettingsPreview : UserControl
{
    public OsdViewModel PreviewViewModel { get; }

    public SettingsPreview()
    {
        PreviewViewModel = new OsdViewModel
        {
            Label = "Bus A1",
            GainText = "+3.0 dB",
            GainNormalized = 0.85,
        };
        PreviewViewModel.Media.Title = "Sample track";
        PreviewViewModel.Media.Artist = "Sample artist";
        PreviewViewModel.Media.HasSession = true;

        DataContext = PreviewViewModel;
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

    public void UpdateCompact(bool compact) => PreviewViewModel.CompactMode = compact;

    public void UpdateColorThresholds(bool thresholds) => PreviewViewModel.UseColorThresholds = thresholds;

}
