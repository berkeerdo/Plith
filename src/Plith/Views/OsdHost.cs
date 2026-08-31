using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Input;
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

    // Distance in DIPs at which a drag snaps to the nearest 3x3 grid hotspot. Small
    // enough that free positioning still feels free, large enough that the user can
    // reliably lock the OSD to the classic preset corners without pixel-hunting.
    private const double SnapThresholdDip = 40;

    private readonly OsdContent _content;
    private readonly SettingsService _settings;
    private DispatcherTimer? _hideTimer;
    private int _showGeneration;
    private TimeSpan _currentVisibleFor;
    private bool _isFadingOut;

    // Edit-mode state. Only touched from the UI dispatcher.
    private bool _isEditMode;
    private bool _isDragging;
    private Point _dragMouseStart;
    private double _dragOsdStartLeft;
    private double _dragOsdStartTop;
    private SettingsModel? _preEditSnapshot;
    private bool _preEditClickThrough;

    public OsdViewModel ViewModel { get; } = new();

    public event EventHandler<MediaCommand>? MediaCommandInvoked;

    /// <summary>Raised when the OSD enters or exits position-edit mode so Settings
    /// can flip its Edit/Save/Cancel button state.</summary>
    public event Action<bool>? EditModeChanged;

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
        if (_isEditMode) return;
        if (!_settings.Current.HoverKeepAlive) return;
        _hideTimer?.Stop();
        BeginAnimation(OpacityProperty, null);
        Opacity = Math.Clamp(_settings.Current.OsdOpacityPercent, 50, 100) / 100.0;
    }

    private void OnMouseLeave(object sender, System.Windows.Input.MouseEventArgs e)
    {
        if (_isEditMode) return;
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
        if (_isEditMode) return;   // edit mode keeps its own always-on visibility
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
        var m = _settings.Current;
        var screen = ResolveTargetScreen(m);
        if (screen is null) return;
        var area = screen.WorkingArea;

        _content.Measure(new Size(double.PositiveInfinity, double.PositiveInfinity));
        _content.UpdateLayout();
        var w = _content.DesiredSize.Width;
        var h = _content.DesiredSize.Height;
        if (w == 0 || h == 0) return;

        (Left, Top) = m.Position switch
        {
            OsdPosition.BottomCenter => (area.Left + (area.Width - w) / 2, area.Bottom - h - EdgeMarginDip),
            OsdPosition.BottomRight  => (area.Right - w - EdgeMarginDip,   area.Bottom - h - EdgeMarginDip),
            OsdPosition.TopCenter    => (area.Left + (area.Width - w) / 2, area.Top + EdgeMarginDip),
            OsdPosition.TopRight     => (area.Right - w - EdgeMarginDip,   area.Top + EdgeMarginDip),
            OsdPosition.Custom       => CustomAnchor(area, w, h, m.CustomPositionXPercent, m.CustomPositionYPercent),
            _                        => (area.Left + (area.Width - w) / 2, area.Bottom - h - EdgeMarginDip),
        };
    }

    // Custom is stored as 0..1 fractions of the placeable range (working area minus OSD
    // size). Clamping to [0,1] tolerates a resolution shrink between saving and restoring
    // without the OSD ending up drawn off-screen.
    private static (double left, double top) CustomAnchor(Rect area, double w, double h, double px, double py)
    {
        var placeableW = Math.Max(0, area.Width - w);
        var placeableH = Math.Max(0, area.Height - h);
        px = Math.Clamp(px, 0.0, 1.0);
        py = Math.Clamp(py, 0.0, 1.0);
        return (area.Left + placeableW * px, area.Top + placeableH * py);
    }

    // Choose the monitor a Custom-positioned OSD anchors on. Match by device name so a
    // resolution or scaling change on the same physical display keeps the OSD there.
    // Fall back to primary when the saved monitor is unplugged (external display gone).
    private static Screen? ResolveTargetScreen(SettingsModel m)
    {
        if (m.Position == OsdPosition.Custom && !string.IsNullOrEmpty(m.CustomPositionMonitorDeviceName))
        {
            foreach (var s in Screen.AllScreens)
            {
                if (string.Equals(s.DeviceName, m.CustomPositionMonitorDeviceName, StringComparison.OrdinalIgnoreCase))
                    return s;
            }
        }
        return Screen.PrimaryScreen;
    }

    // ============ Position edit mode ============

    /// <summary>True while the user is placing the OSD by dragging. Kept as a property
    /// so Settings can bind Save/Cancel button visibility to it.</summary>
    public bool IsInEditMode => _isEditMode;

    /// <summary>Enter drag-to-position mode: pin the OSD visible, capture drag input on
    /// the content, and stop hover/fade behaviour. If the user was on a preset, seed the
    /// current pixel position from that preset so the OSD sits where they expect before
    /// the first move.</summary>
    public void EnterPositionEditMode()
    {
        if (_isEditMode) return;
        _isEditMode = true;
        _preEditSnapshot = _settings.Current.Clone();
        _preEditClickThrough = IsClickThrough;

        _hideTimer?.Stop();
        _isFadingOut = false;

        // Make sure the OSD is on-screen at whatever its current preset would render.
        Reposition();

        // Show fully opaque so the user sees exactly what visitors will see.
        BeginAnimation(OpacityProperty, null);
        Opacity = Math.Clamp(_settings.Current.OsdOpacityPercent, 50, 100) / 100.0;
        Show();
        ReassertTopmost();

        IsClickThrough = false;
        _content.Cursor = Cursors.SizeAll;
        _content.MouseLeftButtonDown += OnEditMouseDown;
        _content.MouseMove += OnEditMouseMove;
        _content.MouseLeftButtonUp += OnEditMouseUp;

        EditModeChanged?.Invoke(true);
    }

    /// <summary>Leave edit mode. When <paramref name="save"/> is true the current pixel
    /// position is translated into monitor-relative fractions and persisted; when false
    /// the pre-edit settings snapshot is restored.</summary>
    public void ExitPositionEditMode(bool save)
    {
        if (!_isEditMode) return;

        _content.MouseLeftButtonDown -= OnEditMouseDown;
        _content.MouseMove -= OnEditMouseMove;
        _content.MouseLeftButtonUp -= OnEditMouseUp;
        if (_content.IsMouseCaptured) _content.ReleaseMouseCapture();
        _content.Cursor = null;
        IsClickThrough = _preEditClickThrough;
        _isDragging = false;
        _isEditMode = false;

        if (save)
        {
            PersistCurrentPositionAsCustom();
        }
        else if (_preEditSnapshot is not null)
        {
            _settings.Save(_preEditSnapshot);
        }
        _preEditSnapshot = null;

        EditModeChanged?.Invoke(false);

        // Give the user a short preview at the new position before it fades.
        ShowOsd(TimeSpan.FromMilliseconds(Math.Max(_settings.Current.ShowDurationMs, 1500)));
    }

    private void OnEditMouseDown(object sender, MouseButtonEventArgs e)
    {
        _isDragging = true;
        _dragMouseStart = e.GetPosition(this);
        _dragOsdStartLeft = Left;
        _dragOsdStartTop = Top;
        _content.CaptureMouse();
        e.Handled = true;
    }

    private void OnEditMouseMove(object sender, System.Windows.Input.MouseEventArgs e)
    {
        if (!_isDragging) return;
        var current = e.GetPosition(this);
        var dx = current.X - _dragMouseStart.X;
        var dy = current.Y - _dragMouseStart.Y;

        var proposedLeft = _dragOsdStartLeft + dx;
        var proposedTop = _dragOsdStartTop + dy;

        var screen = ResolveEditScreenForPoint(proposedLeft, proposedTop);
        if (screen is null) return;
        var area = screen.WorkingArea;

        var w = _content.ActualWidth;
        var h = _content.ActualHeight;

        // Snap to the 3x3 grid unless Alt is held (free positioning for fine-tuning).
        bool freeMove = (Keyboard.Modifiers & ModifierKeys.Alt) != 0;
        if (!freeMove)
        {
            (proposedLeft, proposedTop) = SnapToGrid(area, w, h, proposedLeft, proposedTop);
        }

        // Never let the user drag the OSD off the screen entirely.
        proposedLeft = Math.Clamp(proposedLeft, area.Left, Math.Max(area.Left, area.Right - w));
        proposedTop = Math.Clamp(proposedTop, area.Top, Math.Max(area.Top, area.Bottom - h));

        Left = proposedLeft;
        Top = proposedTop;
    }

    private void OnEditMouseUp(object sender, MouseButtonEventArgs e)
    {
        if (!_isDragging) return;
        _isDragging = false;
        if (_content.IsMouseCaptured) _content.ReleaseMouseCapture();
        e.Handled = true;
    }

    // Nine hotspots per monitor: {edge, center, opposite-edge} on each axis with a
    // constant EdgeMarginDip inset so snapped positions never sit right against the
    // taskbar. Snap only fires when the current coordinate is within SnapThresholdDip.
    private static (double left, double top) SnapToGrid(Rect area, double w, double h, double left, double top)
    {
        double[] xTargets =
        {
            area.Left + EdgeMarginDip,                            // left column
            area.Left + (area.Width - w) / 2,                     // center column
            area.Right - w - EdgeMarginDip,                       // right column
        };
        double[] yTargets =
        {
            area.Top + EdgeMarginDip,                             // top row
            area.Top + (area.Height - h) / 2,                     // middle row
            area.Bottom - h - EdgeMarginDip,                      // bottom row
        };

        foreach (var x in xTargets)
            if (Math.Abs(left - x) < SnapThresholdDip) { left = x; break; }
        foreach (var y in yTargets)
            if (Math.Abs(top - y) < SnapThresholdDip) { top = y; break; }
        return (left, top);
    }

    // WpfScreenHelper's Screen.FromPoint expects DIP coordinates, matching the DIP-space
    // Left/Top used throughout this file. Multi-monitor: as the drag crosses a bezel the
    // resolved screen flips, and the snap grid + clamps switch to the new monitor.
    private static Screen? ResolveEditScreenForPoint(double left, double top)
    {
        try { return Screen.FromPoint(new Point(left, top)); }
        catch { return Screen.PrimaryScreen; }
    }

    private void PersistCurrentPositionAsCustom()
    {
        var screen = ResolveEditScreenForPoint(Left, Top) ?? Screen.PrimaryScreen;
        if (screen is null) return;
        var area = screen.WorkingArea;
        var w = _content.ActualWidth;
        var h = _content.ActualHeight;
        var placeableW = Math.Max(1, area.Width - w);
        var placeableH = Math.Max(1, area.Height - h);

        var m = _settings.Current.Clone();
        m.Position = OsdPosition.Custom;
        m.CustomPositionXPercent = Math.Clamp((Left - area.Left) / placeableW, 0.0, 1.0);
        m.CustomPositionYPercent = Math.Clamp((Top - area.Top) / placeableH, 0.0, 1.0);
        m.CustomPositionMonitorDeviceName = screen.DeviceName ?? string.Empty;
        _settings.Save(m);
    }
}
