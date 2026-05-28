using System.Windows;
using System.Windows.Media;
using System.Windows.Media.Animation;
using System.Windows.Threading;
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
    private const double BottomMarginDip = 96;
    private const int FadeInMs = 140;
    private const int FadeOutMs = 220;

    private readonly OsdContent _content;
    private DispatcherTimer? _hideTimer;
    private int _showGeneration;
    private TimeSpan _currentVisibleFor;
    private bool _isFadingOut;

    public OsdViewModel ViewModel { get; } = new();

    public event EventHandler<MediaCommand>? MediaCommandInvoked;

    public OsdWindow()
    {
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

        // Show the window once (invisible, opacity 0) so the native handle is created and
        // first-frame measurement is done before the user triggers a change.
        Loaded += (_, _) => Reposition();

        // Hover keeps the OSD alive: pause auto-hide while the user is interacting, and
        // restart the timer with a fresh full duration when the mouse leaves.
        MouseEnter += OnMouseEnter;
        MouseLeave += OnMouseLeave;
    }

    private void OnMouseEnter(object sender, System.Windows.Input.MouseEventArgs e)
    {
        _hideTimer?.Stop();
        // Cancel any in-flight fade-out so hovering mid-fade pulls the OSD back to full opacity.
        BeginAnimation(OpacityProperty, null);
        Opacity = 1.0;
    }

    private void OnMouseLeave(object sender, System.Windows.Input.MouseEventArgs e)
    {
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
        // only zeroes Opacity), so the handle is always alive by the time we get here. No need
        // to call Show() defensively — and skipping it avoids any latent BandWindow-vs-Window
        // interaction if the Phase 4 game-mode path is ever wired back in.

        // Bump generation so any in-flight fade-out Completed handler from a previous cycle
        // sees a stale generation and skips its reset — otherwise rapid re-trigger blanks the OSD.
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
            // Cancel any in-flight fade-out so a fresh value re-anchors the OSD at full opacity.
            BeginAnimation(OpacityProperty, null);
            Opacity = 1.0;
        }

        // If the mouse is hovering over the OSD, don't arm the hide timer — wait for MouseLeave.
        if (IsMouseOver) return;
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
            // Keep the window alive (just hidden) so the next Show is instant — avoid Hide()
            // since that triggers a full close/reopen cycle. Just leave Opacity at 0.
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

        // UpdateLayout forces measurement so DesiredSize/ActualWidth are correct on first call.
        _content.Measure(new Size(double.PositiveInfinity, double.PositiveInfinity));
        _content.UpdateLayout();
        var w = _content.DesiredSize.Width;
        var h = _content.DesiredSize.Height;
        if (w == 0 || h == 0) return;

        Left = area.Left + (area.Width - w) / 2;
        Top = area.Bottom - h - BottomMarginDip;
    }
}
