using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Media.Animation;
using System.Windows.Threading;
using Plith.Interop;
using Plith.Services;
using Plith.ViewModels;
using WpfScreenHelper;

namespace Plith.Views;

/// <summary>
/// BandWindow-backed OSD host. Creates a native HWND via CreateWindowInBand in the
/// highest z-band the current process is allowed to enter (UIAccess when granted,
/// Desktop otherwise). Replaces the Phase 1 OsdWindow : Window approach, which
/// could not draw above exclusive fullscreen games.
/// </summary>
public sealed class OsdHost : BandWindow
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

    public OsdHost(SettingsService settings)
    {
        _settings = settings;

        ZBandID = NativeMethods.GetTopMostZBandID();
        TopMost = true;
        Activatable = false;      // never steal focus
        IsClickThrough = false;   // mouse hover keep-alive needs hit-testing
        Opacity = 0;
        Focusable = false;

        _content = new OsdContent { DataContext = ViewModel };
        _content.MediaCommandInvoked += (s, cmd) => MediaCommandInvoked?.Invoke(this, cmd);
        Content = _content;

        MouseEnter += OnMouseEnter;
        MouseLeave += OnMouseLeave;

        // Propagate live-preview changes from Settings into the view-model so the OSD
        // updates without requiring a pop to materialise them. Same pattern as OsdWindow.
        _settings.Changed += m => Dispatcher.BeginInvoke(() =>
        {
            ViewModel.UseColorThresholds = m.UseColorThresholds;
            ViewModel.CompactMode = m.CompactMode;
            Reposition();
        });

        ViewModel.UseColorThresholds = _settings.Current.UseColorThresholds;
        ViewModel.CompactMode = _settings.Current.CompactMode;

        // Pre-create the native HWND so the first ShowOsd is instant.
        // BandWindow.CreateWindow is idempotent if HasSourceCreated is already true.
        CreateWindow();
    }

    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool SetWindowPos(nint hWnd, nint hWndInsertAfter, int X, int Y, int cx, int cy, uint uFlags);

    private static readonly nint HWND_TOPMOST = -1;
    private const uint SWP_NOSIZE = 0x0001;
    private const uint SWP_NOMOVE = 0x0002;
    private const uint SWP_NOACTIVATE = 0x0010;

    /// <summary>Re-assert HWND_TOPMOST so a game / video player that raised itself topmost
    /// after our last ShowOsd doesn't sit above us. Safe to call repeatedly: SetWindowPos
    /// with NOACTIVATE leaves the foreground window's focus untouched.</summary>
    public void ReassertTopmost()
    {
        if (Handle == 0) return;
        _ = SetWindowPos(Handle, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
    }

    private void OnMouseEnter(object sender, System.Windows.Input.MouseEventArgs e)
    {
        if (!_settings.Current.HoverKeepAlive) return;
        _hideTimer?.Stop();
        BeginAnimation(OpacityProperty, null);
        Opacity = Math.Clamp(_settings.Current.OsdOpacityPercent, 50, 100) / 100.0;
    }

    private void OnMouseLeave(object sender, System.Windows.Input.MouseEventArgs e)
    {
        if (!_settings.Current.HoverKeepAlive) return;
        if (_currentVisibleFor <= TimeSpan.Zero) return;
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
        _showGeneration++;
        _isFadingOut = false;
        _currentVisibleFor = visibleFor;
        double targetOpacity = Math.Clamp(_settings.Current.OsdOpacityPercent, 50, 100) / 100.0;
        bool wasHidden = Opacity < targetOpacity - 0.01;

        if (wasHidden)
        {
            Reposition();
            Show();   // BandWindow.Show — Visibility=Visible + SetWindowPos HWND_TOPMOST
            BeginAnimation(OpacityProperty, null);
            var fadeIn = new DoubleAnimation(targetOpacity, TimeSpan.FromMilliseconds(FadeInMs))
            {
                EasingFunction = new CubicEase { EasingMode = EasingMode.EaseOut },
            };
            BeginAnimation(OpacityProperty, fadeIn);
        }
        else
        {
            BeginAnimation(OpacityProperty, null);
            Opacity = targetOpacity;
        }

        ReassertTopmost();

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
