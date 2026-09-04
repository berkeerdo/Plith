using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Input;
using System.Windows.Media.Animation;
using System.Windows.Threading;
using Plith.Cards;
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

    // Magnet radius: while dragging or free-clicking, the OSD's center snaps to the
    // nearest 3x3 hotspot when it's within this many DIPs. Roomy enough for a strong
    // "pull" feel without preventing fine placement (Alt bypasses it completely).
    private const double SnapThresholdDip = 80;

    private readonly OsdContent _content;
    private readonly SettingsService _settings;
    private readonly ThemeService _theme;
    private readonly CardHost _cardHost;
    private readonly DiagnosticLog? _log = new();
    private DispatcherTimer? _hideTimer;
    private int _showGeneration;
    private TimeSpan _currentVisibleFor;
    private bool _isFadingOut;
    private bool _isFadingIn;
    // Separate from _showGeneration on purpose: _showGeneration ticks on every ShowOsd call,
    // including the ones that deliberately leave a running fade-in alone, so using it to
    // decide whether a fade-in is still current would strand _isFadingIn at true forever.
    private int _fadeInGeneration;

    // Local mirror of ThemeService.BuildAccentOverride(). Lives on this ContentControl's
    // own Resources.MergedDictionaries so DynamicResource lookups inside OsdContent hit
    // it before walking up. See the comment on ThemeService.BuildAccentOverride for why
    // we can't rely on Application.Resources changes reaching an HwndSource RootVisual.
    private ResourceDictionary? _accentOverride;

    // Edit-mode state. Only touched from the UI dispatcher.
    private bool _isEditMode;
    private SettingsModel? _preEditSnapshot;
    private readonly List<PositionOverlayWindow> _overlays = new();

    /// <summary>Binding root for the OSD content. Exposes CardHost's visible-card list;
    /// the shell itself holds no per-card state.</summary>
    public OsdShellViewModel Shell { get; }

    /// <summary>Raised when the OSD enters or exits position-edit mode so Settings
    /// can flip its Edit/Save/Cancel button state.</summary>
    public event Action<bool>? EditModeChanged;

    public OsdHost(SettingsService settings, ThemeService theme, CardHost cardHost)
    {
        _settings = settings;
        _theme = theme;
        _cardHost = cardHost;
        Shell = new OsdShellViewModel(cardHost);

        ZBandID = NativeMethods.GetTopMostZBandID();
        // Recorded once at startup because it silently decides whether the OSD can cover an
        // exclusive-fullscreen game, and nothing else in the log reveals it. See UiAccess.
        _log?.Info("OsdHost", Plith.Interop.UiAccess.Describe());
        TopMost = true;
        Activatable = false;      // never steal focus
        IsClickThrough = false;   // mouse hover keep-alive needs hit-testing
        Opacity = 0;
        Focusable = false;

        _content = new OsdContent { DataContext = Shell };
        Content = _content;

        // Seed the local accent mirror BEFORE the HwndSource is created (in CreateWindow
        // below) so the very first paint already uses the picked accent. Subsequent
        // updates come from ThemeService.ThemeApplied.
        RefreshAccentMirror();
        _theme.ThemeApplied += OnThemeApplied;

        MouseEnter += OnMouseEnter;
        MouseLeave += OnMouseLeave;

        // A settings save can change where the OSD sits (position / monitor), so re-anchor it
        // without waiting for the next pop. Card-level settings (colour thresholds, compact
        // mode) are owned by AudioCard and MediaCard and never travel through the shell.
        _settings.Changed += _ => Dispatcher.BeginInvoke(() => Reposition());

        // Pre-create the native HWND so the first ShowOsd is instant.
        // BandWindow.CreateWindow is idempotent if HasSourceCreated is already true.
        CreateWindow();
    }

    private void OnThemeApplied()
    {
        // Marshal to the UI thread in case ThemeApplied ever fires from a different
        // context (Windows preference watcher already dispatches, but defend anyway).
        if (!Dispatcher.CheckAccess())
        {
            Dispatcher.BeginInvoke(new Action(RefreshAccentMirror));
            return;
        }
        RefreshAccentMirror();
    }

    private void RefreshAccentMirror()
    {
        var dict = _theme.BuildAccentOverride();
        if (_accentOverride is not null)
            Resources.MergedDictionaries.Remove(_accentOverride);
        Resources.MergedDictionaries.Add(dict);
        _accentOverride = dict;
        // The tinted surface changes the card padding illusion but doesn't move geometry;
        // still, force a re-measure so a redraw is queued right away rather than waiting
        // for the next natural render pass. Cheap — no layout invalidation upstream.
        _content?.InvalidateVisual();
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
        // CardHost is the single authority for when the OSD appears. Whether a
        // faded-out (Opacity 0) layered window still hit-tests mouse messages has never
        // been verified — if it does, hovering over the OSD's screen region while
        // fullscreen-video suppression is active would resurrect it here, bypassing the
        // suppressor entirely. Guard defensively rather than find out live.
        if (_cardHost.Suppressor?.IsSuppressed == true) return;
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
        bool wasFadingOut = _isFadingOut;
        _isFadingOut = false;
        _currentVisibleFor = visibleFor;
        double targetOpacity = Math.Clamp(_settings.Current.OsdOpacityPercent, 50, 100) / 100.0;
        bool wasHidden = Opacity < targetOpacity - 0.01;

        if (wasHidden)
        {
            Reposition();
            Show();   // BandWindow.Show — Visibility=Visible + SetWindowPos HWND_TOPMOST

            // Only start a fade-in when one is not already running toward this same target.
            // Volume keys repeat far faster than FadeInMs, so a held or spammed key lands
            // several events inside a single fade — and restarting the animation on each of
            // them made the OSD pulse instead of staying up. Note also the absence of a
            // BeginAnimation(OpacityProperty, null) here: clearing an animation reverts the
            // property to its base value, which is 0 while the window is hidden, so the old
            // clear-then-restart snapped the OSD back to fully invisible every time. A
            // From-less DoubleAnimation hands off from the current animated value instead,
            // which is what makes interrupting a fade-out look continuous.
            if (wasFadingOut || !_isFadingIn)
            {
                // Logged on the transition only, not on every repeat, so a held volume key
                // produces one line per appearance. This is the line that separates "the OSD
                // was never asked to show" from "it was shown and something on top of it won":
                // over a game in true exclusive fullscreen the display is scanned out from the
                // game's own swapchain, so nothing composites over it however correctly the
                // OSD behaves, and without this line the two cases look identical from a log.
                _log?.Info("OsdHost",
                    $"Show: fade-in at {Left:0},{Top:0} for {visibleFor.TotalMilliseconds:0}ms" +
                    (wasFadingOut ? " (interrupting fade-out)" : string.Empty));

                _isFadingIn = true;
                int gen = ++_fadeInGeneration;
                var fadeIn = new DoubleAnimation(targetOpacity, TimeSpan.FromMilliseconds(FadeInMs))
                {
                    EasingFunction = new CubicEase { EasingMode = EasingMode.EaseOut },
                };
                fadeIn.Completed += (_, _) =>
                {
                    if (_fadeInGeneration == gen) _isFadingIn = false;
                };
                BeginAnimation(OpacityProperty, fadeIn);
            }
        }
        else
        {
            BeginAnimation(OpacityProperty, null);
            Opacity = targetOpacity;
            _isFadingIn = false;
        }

        ReassertTopmost();

        if (_settings.Current.HoverKeepAlive && IsMouseOver) return;
        RestartHideTimer(visibleFor);
    }

    /// <summary>Take the OSD down now, ignoring its hide timer. Used when the suppression gate
    /// closes while the card is already on screen.</summary>
    public void HideOsd()
    {
        if (_isEditMode) return;   // edit mode owns its own always-on visibility
        _hideTimer?.Stop();
        if (Opacity < 0.01) return;
        // Already on the way out — restarting the animation from the current opacity would
        // stretch the fade instead of shortening it.
        if (_isFadingOut) return;
        FadeOutAndHide();
    }

    private void FadeOutAndHide()
    {
        var gen = _showGeneration;
        _isFadingOut = true;
        _isFadingIn = false;
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

    // Custom is stored as 0..1 fractions of the monitor working area, marking the
    // CENTRE of the OSD (not its top-left corner). This keeps the OSD visually
    // anchored when its content grows or shrinks (media card appearing / going away):
    // the centre stays fixed and the edges expand/contract symmetrically, so a card
    // dragged to the right side never overflows the screen when a longer media title
    // makes it wider. Clamped to a legal top-left range after applying the centre so
    // the OSD stays on-screen if the resolution shrinks or the content maxes out.
    private static (double left, double top) CustomAnchor(Rect area, double w, double h, double px, double py)
    {
        px = Math.Clamp(px, 0.0, 1.0);
        py = Math.Clamp(py, 0.0, 1.0);
        var centerX = area.Left + area.Width * px;
        var centerY = area.Top + area.Height * py;
        var left = Math.Clamp(centerX - w / 2, area.Left, Math.Max(area.Left, area.Right - w));
        var top = Math.Clamp(centerY - h / 2, area.Top, Math.Max(area.Top, area.Bottom - h));
        return (left, top);
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

    /// <summary>Enter overlay-driven position mode: dim every monitor with a
    /// PositionOverlayWindow (grid hotspots, Save/Cancel toolbar, and full drag
    /// detection over the OSD rectangle). Overlay tells the OSD where to move by
    /// firing PositionRequested; OsdHost applies snap/clamp and pushes the new
    /// rectangle back to every overlay so drag hit-testing stays accurate.</summary>
    public void EnterPositionEditMode()
    {
        if (_isEditMode) return;
        _isEditMode = true;
        _preEditSnapshot = _settings.Current.Clone();

        _hideTimer?.Stop();
        _isFadingOut = false;

        // Full opacity so the OSD is unmistakably visible above the dim layer.
        BeginAnimation(OpacityProperty, null);
        Opacity = Math.Clamp(_settings.Current.OsdOpacityPercent, 50, 100) / 100.0;
        Reposition();
        Show();
        ReassertTopmost();

        // Drag handling lives ENTIRELY inside PositionOverlayWindow: the overlay
        // owns the click surface (we know it does because hotspots work) and simply
        // checks whether the mouse-down landed inside the OSD's current rectangle.
        // That means the OSD itself does not need to receive clicks - keep it in
        // its normal click-through state so we do not fight the DWM/LAYERED hit-
        // test rules the OSD was originally built for.
        _log?.Info("OsdHost", "EnterPositionEditMode - drag lives on overlay canvas");

        // One overlay per monitor - hotspots and toolbar. Note: in UIAccess=false
        // builds the OSD is a regular Topmost window (not a band window), so the
        // last-shown Topmost wins the z-race. Overlays would end up ABOVE the OSD,
        // eating every click before it can reach the drag handlers - the reason
        // drag would appear dead while hotspot clicks still worked. Reassert OSD
        // topmost AFTER the overlays are created so the click surface goes back on
        // top of the dim layer.
        foreach (var screen in Screen.AllScreens)
        {
            var overlay = new PositionOverlayWindow(screen);
            overlay.PositionRequested += OnOverlayClickRequested;
            overlay.SaveRequested += OnOverlaySaveRequested;
            overlay.CancelRequested += OnOverlayCancelRequested;
            overlay.Show();
            _overlays.Add(overlay);
        }
        ReassertTopmost();
        BroadcastOsdRect();
        _log?.Info("OsdHost", $"overlays shown ({_overlays.Count}); OSD topmost reasserted; rect published");

        // Focus lands on the primary overlay so Esc / Enter shortcuts fire immediately.
        var primary = Screen.PrimaryScreen?.WorkingArea.Left ?? 0;
        var focus = _overlays.Find(o => Math.Abs(o.Left - primary) < 0.5) ?? _overlays[0];
        focus.Activate();
        _ = focus.Focus();

        EditModeChanged?.Invoke(true);
    }

    /// <summary>Programmatic exit from position edit mode. When <paramref name="save"/>
    /// is true the current OSD position is persisted as Custom (centre-anchored
    /// fractions), otherwise the pre-edit settings snapshot is restored.</summary>
    public void ExitPositionEditMode(bool save)
    {
        if (!_isEditMode) return;

        foreach (var overlay in _overlays)
        {
            overlay.PositionRequested -= OnOverlayClickRequested;
            overlay.SaveRequested -= OnOverlaySaveRequested;
            overlay.CancelRequested -= OnOverlayCancelRequested;
            overlay.Close();
        }
        _overlays.Clear();

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

        _cardHost.RequestShow(new ShowRequest(
            ShowReason.EditModeExit,
            null,
            TimeSpan.FromMilliseconds(Math.Max(_settings.Current.ShowDurationMs, 1500))));
    }

    private void OnOverlaySaveRequested() => ExitPositionEditMode(save: true);
    private void OnOverlayCancelRequested() => ExitPositionEditMode(save: false);

    // Overlay is the source of truth for snap decisions now (it already knows the
    // OSD rectangle from UpdateOsdRect). Whatever centre it emits is applied as-is;
    // mid-drag moves are unsnapped for smoothness, release-time and hotspot clicks
    // arrive already snapped to their target.
    private void OnOverlayClickRequested(Screen screen, Point absoluteCenter)
    {
        var (w, h) = MeasuredOsdSize();
        SetOsdCenteredAt(screen.WorkingArea, w, h, absoluteCenter.X, absoluteCenter.Y);
    }

    // Nine snap targets = {EdgeMarginDip, center, opposite-EdgeMarginDip} on each axis.
    // Target values are OSD CENTRES (matching the persistence semantics), so a corner
    // hotspot's centre sits EdgeMarginDip + w/2 from the working-area edge.
    private static (double cx, double cy) MaybeSnapCenter(Rect area, double w, double h, double cx, double cy)
    {
        // Alt = free placement, no magnet.
        if ((Keyboard.Modifiers & ModifierKeys.Alt) != 0) return (cx, cy);

        double[] cxTargets =
        {
            area.Left + EdgeMarginDip + w / 2,       // left column centre
            area.Left + area.Width / 2,               // centre column centre
            area.Right - EdgeMarginDip - w / 2,       // right column centre
        };
        double[] cyTargets =
        {
            area.Top + EdgeMarginDip + h / 2,        // top row centre
            area.Top + area.Height / 2,               // middle row centre
            area.Bottom - EdgeMarginDip - h / 2,      // bottom row centre
        };
        foreach (var tx in cxTargets)
            if (Math.Abs(cx - tx) < SnapThresholdDip) { cx = tx; break; }
        foreach (var ty in cyTargets)
            if (Math.Abs(cy - ty) < SnapThresholdDip) { cy = ty; break; }
        return (cx, cy);
    }

    private void SetOsdCenteredAt(Rect area, double w, double h, double cx, double cy)
    {
        var left = Math.Clamp(cx - w / 2, area.Left, Math.Max(area.Left, area.Right - w));
        var top = Math.Clamp(cy - h / 2, area.Top, Math.Max(area.Top, area.Bottom - h));
        Left = left;
        Top = top;
        ReassertTopmost();
        BroadcastOsdRect();
    }

    // Push the OSD's current absolute rectangle to every overlay so their drag hit-
    // test knows where the card sits. Called on Enter and after every reposition.
    private void BroadcastOsdRect()
    {
        var (w, h) = MeasuredOsdSize();
        var rect = new Rect(Left, Top, w, h);
        foreach (var overlay in _overlays) overlay.UpdateOsdRect(rect);
    }

    private (double w, double h) MeasuredOsdSize()
    {
        var w = _content.ActualWidth > 0 ? _content.ActualWidth : _content.DesiredSize.Width;
        var h = _content.ActualHeight > 0 ? _content.ActualHeight : _content.DesiredSize.Height;
        if (w == 0 || h == 0)
        {
            _content.Measure(new Size(double.PositiveInfinity, double.PositiveInfinity));
            _content.UpdateLayout();
            w = _content.DesiredSize.Width;
            h = _content.DesiredSize.Height;
        }
        return (w, h);
    }

    // Store the OSD's CENTRE as a fraction of the target monitor's working area (not
    // the top-left corner). Reposition() rebuilds Left/Top from those fractions using
    // the OSD's current size, so a content-size change (media card appearing) leaves
    // the OSD visually anchored on the same point.
    private void PersistCurrentPositionAsCustom()
    {
        var (w, h) = MeasuredOsdSize();
        var centerX = Left + w / 2;
        var centerY = Top + h / 2;
        var screen = Screen.FromPoint(new Point(centerX, centerY)) ?? Screen.PrimaryScreen;
        if (screen is null) return;
        var area = screen.WorkingArea;

        var m = _settings.Current.Clone();
        m.Position = OsdPosition.Custom;
        m.CustomPositionXPercent = Math.Clamp((centerX - area.Left) / area.Width, 0.0, 1.0);
        m.CustomPositionYPercent = Math.Clamp((centerY - area.Top) / area.Height, 0.0, 1.0);
        m.CustomPositionMonitorDeviceName = screen.DeviceName ?? string.Empty;
        _settings.Save(m);
    }
}
