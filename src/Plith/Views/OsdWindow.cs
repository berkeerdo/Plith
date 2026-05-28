using System.Windows;
using System.Windows.Media;
using System.Windows.Media.Animation;
using System.Windows.Threading;
using Plith.Services;
using Plith.ViewModels;
using WpfScreenHelper;

namespace Plith.Views;

/// <summary>
/// Regular topmost WPF window hosting the OSD. Uses per-pixel transparency to render the
/// rounded card shape, ShowInTaskbar=false + ShowActivated=false to stay invisible to the
/// taskbar/Alt+Tab and avoid stealing focus from games.
///
/// Note: this does not draw above fullscreen-exclusive games (would need the BandWindow
/// + UIAccess path — deferred to Phase 4 "game mode"). It does draw over fullscreen-borderless,
/// which is what nearly all modern titles use.
/// </summary>
public sealed class OsdWindow : Window
{
    private const double EdgeMarginDip = 96;
    private const int FadeInMs = 140;
    private const int FadeOutMs = 220;

    private readonly OsdContent _content;
    private readonly SettingsService _settings;
    private DispatcherTimer? _hideTimer;
    private int _showGeneration;
    private TimeSpan _currentVisibleFor;
    private bool _isFadingOut;

    public OsdViewModel ViewModel { get; } = new();

    public event EventHandler<MediaCommand>? MediaCommandInvoked;

    public OsdWindow(SettingsService settings)
    {
        _settings = settings;

        WindowStyle = WindowStyle.None;
        AllowsTransparency = true;
        Background = Brushes.Transparent;
        Topmost = true;
        ShowInTaskbar = false;
        ShowActivated = false;
        ResizeMode = ResizeMode.NoResize;
        SizeToContent = SizeToContent.WidthAndHeight;
        WindowStartupLocation = WindowStartupLocation.Manual;
        Opacity = 0d;
        Focusable = false;

        _content = new OsdContent { DataContext = ViewModel };
        _content.MediaCommandInvoked += (s, cmd) => MediaCommandInvoked?.Invoke(this, cmd);
        Content = _content;

        Loaded += (_, _) => Reposition();

        // Hover keeps the OSD alive: pause auto-hide while the user is interacting, and
        // restart the timer with a fresh full duration when the mouse leaves.
        MouseEnter += OnMouseEnter;
        MouseLeave += OnMouseLeave;

        // Reposition when the user picks a new position in settings.
        _settings.Changed += _ => Dispatcher.BeginInvoke(Reposition);
    }

    private void OnMouseEnter(object sender, System.Windows.Input.MouseEventArgs e)
    {
        if (!_settings.Current.HoverKeepAlive) return;
        _hideTimer?.Stop();
        BeginAnimation(OpacityProperty, null);
        Opacity = 1.0;
    }

    private void OnMouseLeave(object sender, System.Windows.Input.MouseEventArgs e)
    {
        if (!_settings.Current.HoverKeepAlive) return;
        if (_currentVisibleFor <= TimeSpan.Zero) return;
        // If we're already fading out (timer expired before the user moved away), don't
        // re-arm — that would launch a second opacity animation that competes with the
        // in-flight fade-out and burns a wasted DispatcherTimer allocation.
        if (_isFadingOut) return;
        RestartHideTimer(_currentVisibleFor);
    }

    private void RestartHideTimer(TimeSpan visibleFor)
    {
        _hideTimer?.Stop();
        _hideTimer = new DispatcherTimer { Interval = visibleFor };
        _hideTimer.Tick += (_, _) =>
        {
            _hideTimer!.Stop();
            FadeOutAndHide();
        };
        _hideTimer.Start();
    }

    public void ShowOsd(TimeSpan visibleFor)
    {
        // App.xaml.cs Show()s the window once at startup and we never Hide() it (FadeOutAndHide
        // only zeroes Opacity), so the handle is always alive by the time we get here.

        _showGeneration++;
        _isFadingOut = false;
        _currentVisibleFor = visibleFor;
        bool wasHidden = Opacity < 0.99;

        if (wasHidden)
        {
            Reposition();
            BeginAnimation(OpacityProperty, null);
            var fadeIn = new DoubleAnimation(1.0, TimeSpan.FromMilliseconds(FadeInMs))
            {
                EasingFunction = new CubicEase { EasingMode = EasingMode.EaseOut },
            };
            BeginAnimation(OpacityProperty, fadeIn);
        }
        else
        {
            BeginAnimation(OpacityProperty, null);
            Opacity = 1.0;
        }

        if (_settings.Current.HoverKeepAlive && IsMouseOver) return;
        RestartHideTimer(visibleFor);
    }

    private void FadeOutAndHide()
    {
        var gen = _showGeneration;
        _isFadingOut = true;
        var fadeOut = new DoubleAnimation(0.0, TimeSpan.FromMilliseconds(FadeOutMs))
        {
            EasingFunction = new CubicEase { EasingMode = EasingMode.EaseIn },
        };
        fadeOut.Completed += (_, _) =>
        {
            if (_showGeneration == gen)
            {
                Opacity = 0;
                _isFadingOut = false;
            }
        };
        BeginAnimation(OpacityProperty, fadeOut);
    }

    private void Reposition()
    {
        var screen = Screen.PrimaryScreen;
        if (screen is null) return;
        var area = screen.WorkingArea;

        _content.Measure(new Size(double.PositiveInfinity, double.PositiveInfinity));
        _content.UpdateLayout();
        var w = _content.DesiredSize.Width;
        var h = _content.DesiredSize.Height;
        if (w == 0 || h == 0) return;

        (Left, Top) = _settings.Current.Position switch
        {
            OsdPosition.BottomCenter => (area.Left + (area.Width - w) / 2, area.Bottom - h - EdgeMarginDip),
            OsdPosition.BottomRight  => (area.Right - w - EdgeMarginDip,   area.Bottom - h - EdgeMarginDip),
            OsdPosition.TopCenter    => (area.Left + (area.Width - w) / 2, area.Top + EdgeMarginDip),
            OsdPosition.TopRight     => (area.Right - w - EdgeMarginDip,   area.Top + EdgeMarginDip),
            _                        => (area.Left + (area.Width - w) / 2, area.Bottom - h - EdgeMarginDip),
        };
    }
}
