using System.Windows;
using System.Windows.Media.Animation;
using Plith.Interop;
using Plith.ViewModels;
using WpfScreenHelper;

namespace Plith.Views;

/// <summary>
/// The actual on-screen display: a BandWindow that hosts an OsdContent UserControl,
/// positioned at the bottom-center of the primary screen, fading in/out on demand.
/// </summary>
public sealed class OsdWindow : BandWindow
{
    private const double BottomMarginDip = 96;
    private const int FadeInMs = 140;
    private const int FadeOutMs = 220;

    private readonly OsdContent _content;
    private DispatcherTimerWrapper? _hideTimer;
    private int _showGeneration;

    public OsdViewModel ViewModel { get; } = new();

    public OsdWindow()
    {
        IsClickThrough = true;
        TopMost = true;
        Activatable = false;
        ZBandID = NativeMethods.GetTopMostZBandID();
        Opacity = 0d;

        _content = new OsdContent { DataContext = ViewModel };
        Content = _content;

        // Mica intentionally not applied here: it fills the window's entire bounding box
        // with the system backdrop, which shows through the transparent corners of the
        // rounded card and reads as a flat rectangle behind it. The semi-transparent dark
        // surface brush on the inner Border already gives a modern look on its own.
        // Re-enabling Mica properly needs the window itself to be the rounded shape
        // (Win11 DWMWA_WINDOW_CORNER_PREFERENCE) — Phase 4 polish.
        SourceCreated += (_, _) => Reposition();
    }

    public void ShowOsd(TimeSpan visibleFor)
    {
        if (!HasSourceCreated) CreateWindow();

        // Bump generation so any in-flight fade-out Completed handler from a previous cycle
        // sees a stale generation and skips its Hide() — otherwise rapid re-trigger blanks the OSD.
        _showGeneration++;
        bool wasHidden = Visibility != Visibility.Visible || Opacity < 0.99;

        if (wasHidden)
        {
            Reposition();
            Show();
            BeginAnimation(OpacityProperty, null);
            var fadeIn = new DoubleAnimation(1.0, TimeSpan.FromMilliseconds(FadeInMs))
            {
                EasingFunction = new CubicEase { EasingMode = EasingMode.EaseOut },
            };
            BeginAnimation(OpacityProperty, fadeIn);
        }
        else
        {
            // Cancel any in-flight fade-out so a fresh value re-anchors the OSD at full opacity.
            BeginAnimation(OpacityProperty, null);
            Opacity = 1.0;
        }

        _hideTimer?.Stop();
        _hideTimer = new DispatcherTimerWrapper(visibleFor, FadeOutAndHide);
        _hideTimer.Start();
    }

    private void FadeOutAndHide()
    {
        var gen = _showGeneration;
        var fadeOut = new DoubleAnimation(0.0, TimeSpan.FromMilliseconds(FadeOutMs))
        {
            EasingFunction = new CubicEase { EasingMode = EasingMode.EaseIn },
        };
        fadeOut.Completed += (_, _) =>
        {
            if (_showGeneration == gen) Hide();
        };
        BeginAnimation(OpacityProperty, fadeOut);
    }

    private void Reposition()
    {
        var screen = Screen.PrimaryScreen;
        if (screen is null) return;
        var area = screen.WorkingArea;

        // Force a layout pass so DesiredSize is current before positioning.
        // UpdateLayout() must follow Measure() — without it, a control not yet in a live
        // visual tree reports DesiredSize as {0, 0} and the window snaps off-screen.
        _content.Measure(new Size(double.PositiveInfinity, double.PositiveInfinity));
        _content.UpdateLayout();
        var size = _content.DesiredSize;
        if (size.Width == 0 || size.Height == 0) return;

        Left = area.Left + (area.Width - size.Width) / 2;
        Top = area.Bottom - size.Height - BottomMarginDip;
    }
}

internal sealed class DispatcherTimerWrapper
{
    private readonly System.Windows.Threading.DispatcherTimer _timer;
    public DispatcherTimerWrapper(TimeSpan interval, Action onTick)
    {
        _timer = new System.Windows.Threading.DispatcherTimer { Interval = interval };
        _timer.Tick += (_, _) =>
        {
            _timer.Stop();
            onTick();
        };
    }
    public void Start() => _timer.Start();
    public void Stop() => _timer.Stop();
}
