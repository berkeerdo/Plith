// Portions adapted from VoicemeeterFancyOSD (MIT, A-tG and contributors). See NOTICE.md.
using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Interop;
using System.Windows.Media;
using Plith.Services;
using static Plith.Interop.NativeMethods;

namespace Plith.Interop;

public enum ZBandID
{
    Default = 0x0,
    Desktop = 0x1,
    UIAccess = 0x2,
    ImmersiveIHM = 0x3,
    ImmersiveNotification = 0x4,
    ImmersiveAppChrome = 0x5,
    ImmersiveMogo = 0x6,
    ImmersiveEdgy = 0x7,
    ImmersiveInActiveMOBODY = 0x8,
    ImmersiveInActiveDock = 0x9,
    ImmersiveActiveMOBODY = 0xA,
    ImmersiveActiveDock = 0xB,
    ImmersiveBackground = 0xC,
    ImmersiveSearch = 0xD,
    GenuineWindows = 0xE,
    ImmersiveRestricted = 0xF,
    SystemTools = 0x10,
    Lock = 0x11,
    AboveLockUX = 0x12,
}

public delegate nint WndProc(nint hWnd, uint msg, nint wParam, nint lParam);

/// <summary>
/// WPF ContentControl that owns a native Win32 window created via the undocumented
/// <c>CreateWindowInBand</c> API, enabling reliable topmost-over-fullscreen rendering.
/// Falls back to <c>CreateWindowEx</c> when CreateWindowInBand is unavailable.
/// </summary>
// CA1001: BandWindow owns _hwndSource (IDisposable). It's released in OnAppExit
// (App.Exit handler wired in BandWindowExt). The class doesn't implement IDisposable
// because WPF FrameworkElement lifecycle is owned by the visual tree, not by callers.
[System.Diagnostics.CodeAnalysis.SuppressMessage("Design", "CA1001:Types that own disposable fields should be disposable",
    Justification = "_hwndSource is released in the Application.Exit handler wired by BandWindowExt; WPF visual tree owns the rest of the lifecycle.")]
public partial class BandWindow : ContentControl, IWndProcObject
{
    private HwndSource? _hwndSource;
    private double _dpiScale = 1.0;
    private readonly WndProcHookManager _hookManager;
    private bool _isSizeChanging;
    private bool _isVisibilityChanging;

    protected HwndSource? HwndSource => _hwndSource;



    /// <summary>The display scale the window is currently sized against. Exposed so the
    /// partial half can convert DIP rectangles into the physical pixels Win32 wants.</summary>
    protected double CurrentDpiScale => _dpiScale;

    #region DependencyProperties

    public static readonly DependencyProperty ActivatableProperty =
        DependencyProperty.Register(nameof(Activatable), typeof(bool), typeof(BandWindow),
            new PropertyMetadata(false, OnActivatablePropertyChanged));

    public bool Activatable
    {
        get => (bool)GetValue(ActivatableProperty);
        set => SetValue(ActivatableProperty, value);
    }

    private static void OnActivatablePropertyChanged(DependencyObject d, DependencyPropertyChangedEventArgs e)
    {
        if (d is not BandWindow bw || !bw.HasSourceCreated) return;
        if ((bool)e.NewValue)
            ApplyWindowStyles(bw.Handle, wsEXToRemove: ExtendedWindowStyles.WS_EX_NOACTIVATE);
        else
            ApplyWindowStyles(bw.Handle, wsEXToAdd: ExtendedWindowStyles.WS_EX_NOACTIVATE);
    }

    private static readonly DependencyPropertyKey HandlePropertyKey =
        DependencyProperty.RegisterReadOnly(nameof(Handle), typeof(nint), typeof(BandWindow),
            new PropertyMetadata((nint)0));
    public static readonly DependencyProperty HandleProperty = HandlePropertyKey.DependencyProperty;
    public nint Handle
    {
        get => (nint)GetValue(HandleProperty);
        private set => SetValue(HandlePropertyKey, value);
    }

    private static readonly DependencyPropertyKey IsActivePropertyKey =
        DependencyProperty.RegisterReadOnly(nameof(IsActive), typeof(bool), typeof(BandWindow),
            new PropertyMetadata(false));
    public static readonly DependencyProperty IsActiveProperty = IsActivePropertyKey.DependencyProperty;
    public bool IsActive
    {
        get => (bool)GetValue(IsActiveProperty);
        private set => SetValue(IsActivePropertyKey, value);
    }

    private static readonly DependencyPropertyKey HasSourceCreatedPropertyKey =
        DependencyProperty.RegisterReadOnly(nameof(HasSourceCreated), typeof(bool), typeof(BandWindow),
            new PropertyMetadata(false));
    public static readonly DependencyProperty HasSourceCreatedProperty = HasSourceCreatedPropertyKey.DependencyProperty;
    public bool HasSourceCreated
    {
        get => (bool)GetValue(HasSourceCreatedProperty);
        private set => SetValue(HasSourceCreatedPropertyKey, value);
    }

    public static readonly DependencyProperty TopMostProperty =
        DependencyProperty.Register(nameof(TopMost), typeof(bool), typeof(BandWindow),
            new PropertyMetadata(true, OnTopMostPropertyChanged));
    public bool TopMost
    {
        get => (bool)GetValue(TopMostProperty);
        set => SetValue(TopMostProperty, value);
    }

    private static void OnTopMostPropertyChanged(DependencyObject d, DependencyPropertyChangedEventArgs e)
    {
        if (d is not BandWindow bw || !bw.HasSourceCreated) return;
        if ((bool)e.NewValue)
        {
            ApplyWindowStyles(bw.Handle, wsEXToAdd: ExtendedWindowStyles.WS_EX_TOPMOST);
            ShowWindow(bw.Handle, (int)ShowWindowCommands.ShowNoActivate);
        }
        else
        {
            ApplyWindowStyles(bw.Handle, wsEXToRemove: ExtendedWindowStyles.WS_EX_TOPMOST);
        }
    }

    public static readonly DependencyProperty ZBandIDProperty =
        DependencyProperty.Register(nameof(ZBandID), typeof(ZBandID), typeof(BandWindow),
            new PropertyMetadata(ZBandID.Default, OnZBandIDPropertyChanged));
    public ZBandID ZBandID
    {
        get => (ZBandID)GetValue(ZBandIDProperty);
        set => SetValue(ZBandIDProperty, value);
    }

    private static void OnZBandIDPropertyChanged(DependencyObject d, DependencyPropertyChangedEventArgs e)
    {
        if (d is BandWindow bw && bw.HasSourceCreated)
            throw new InvalidOperationException("ZBandID cannot be changed after the window is created.");
    }

    #endregion

    static BandWindow()
    {
        VisibilityProperty.OverrideMetadata(typeof(BandWindow),
            new FrameworkPropertyMetadata(Visibility.Hidden, OnVisibilityPropertyChanged));
    }

    private static void OnVisibilityPropertyChanged(DependencyObject d, DependencyPropertyChangedEventArgs e)
    {
        if (d is BandWindow bw && e.NewValue is Visibility v)
        {
            if (v == Visibility.Visible) bw.Show();
            else bw.Hide();
        }
    }

    public BandWindow()
    {
        SizeChanged += (_, _) => UpdateSize();
        _hookManager = WndProcHookManager.RegisterForIWndProcObject(this);
        BandWindowExt();
    }

    public void CreateWindow()
    {
        if (HasSourceCreated) return;

        // WPF creates the window ITSELF, as a top-level window, and it is then moved into the
        // z-order band afterwards. That ordering is the whole point, and it was arrived at by
        // measurement rather than preference.
        //
        // The previous shape created a container through CreateWindowInBand and hosted WPF in a
        // WS_CHILD HwndSource inside it. That cannot be made to work, because per-pixel opacity
        // is documented to apply only to TOP-LEVEL windows: the child got WS_EX_LAYERED without
        // real per-pixel alpha, and a layered window is hit-tested against its alpha rather than
        // through WM_NCHITTEST. So the system tested an alpha that read opaque across the whole
        // rectangle, and a closed, invisible notch blocked every click in the area the open panel
        // would occupy. Both directions were measured: turning per-pixel transparency off
        // restored hit-testing and painted the notch as a solid black rectangle instead.
        //
        // As a genuine top-level layered window, the alpha WPF renders IS the hit-test mask.
        // Transparent pixels pass the mouse through for free and drawn ones receive it — no
        // WM_NCHITTEST filter, no click-through bit to keep in sync, no window region.
        var extStyles = (int)(
            // TOOLWINDOW is always wanted: this is an overlay, never a primary app window.
            // Without it the OSD shows up in the taskbar and Alt+Tab as if it were a real app.
            ExtendedWindowStyles.WS_EX_TOOLWINDOW |
            (IsClickThrough ? ExtendedWindowStyles.WS_EX_TRANSPARENT : 0) |
            (Activatable ? 0 : ExtendedWindowStyles.WS_EX_NOACTIVATE) |
            (TopMost ? ExtendedWindowStyles.WS_EX_TOPMOST : 0));

        var param = new HwndSourceParameters
        {
            WindowStyle = unchecked((int)((uint)WindowStyles.WS_POPUP | (uint)WindowStyles.WS_VISIBLE)),
            ExtendedWindowStyle = extStyles,
            PositionX = (int)Math.Round(Left),
            PositionY = (int)Math.Round(Top),
            UsesPerPixelTransparency = true,
        };

        _hwndSource = new HwndSource(param)
        {
            SizeToContent = SizeToContent.WidthAndHeight,
            RootVisual = this,
        };
        _hwndSource.CompositionTarget!.BackgroundColor = Colors.Transparent;

        Handle = _hwndSource.Handle;
        if (Handle == 0) throw new Win32Exception(Marshal.GetLastWin32Error());

        // Into the band, now that the window exists. Failure is not fatal — it costs the ability
        // to draw over exclusive-fullscreen games, which is a Phase 4 capability rather than a
        // correctness requirement, and everything else keeps working.
        if (ZBandID != 0 && IsSetWindowBandSupported() && !SetWindowBand(Handle, 0, (uint)ZBandID))
            _bandFailed = true;

        // WPF owns this window's WndProc now, so the messages the old class WndProc handled are
        // taken through the supported hook instead.
        _hwndSource.AddHook(SourceHook);

        OnSourceCreated();
        _hookManager.OnHwndCreated(Handle);
        UpdateWindow(Handle);
        UpdateDpiScale(GetDpiForWindow(Handle) / 96.0);
        HasSourceCreated = true;
    }

    /// <summary>
    /// A sideways wheel gesture, already decoded to a signed delta where positive means "towards
    /// the next page". Raised for a horizontal wheel or a Shift+wheel; a plain vertical wheel
    /// does not raise it. See WheelDecoder for the sign conventions.
    /// </summary>
    public event EventHandler<int>? HorizontalWheel;

    /// <summary>True when the window could not be moved into its z-order band. It still works;
    /// it just cannot draw over an exclusive-fullscreen game.</summary>
    public bool BandFailed => _bandFailed;
    private bool _bandFailed;

    private nint SourceHook(nint hwnd, int msg, nint wParam, nint lParam, ref bool handled)
    {
        var result = MyWndProc(hwnd, (uint)msg, wParam, lParam, ref handled);
        return result;
    }

    protected override void OnDpiChanged(DpiScale oldDpi, DpiScale newDpi)
    {
        base.OnDpiChanged(oldDpi, newDpi);
        UpdateDpiScale(newDpi.DpiScaleX);
    }



    private nint MyWndProc(nint hWnd, uint msg, nint wParam, nint lParam, ref bool handled)
    {
        // Left over from when a WM_NCHITTEST case sat at the top of this switch and decided,
        // per point, whether the window existed for the mouse. It does not any more: the window
        // is layered with per-pixel transparency, so the system hit-tests the alpha WPF rendered
        // and never asks. The filter and the two mechanisms beside it were deleted with it — see
        // docs/PHASE6-VERIFICATION.md §14.
        var message = (WindowMessage)msg;
        switch (message)
        {
            case WindowMessage.WM_ACTIVATE:
                IsActive = wParam.ToInt32() != 0;
                if (IsActive) Activated?.Invoke(this, EventArgs.Empty);
                else Deactivated?.Invoke(this, EventArgs.Empty);
                break;

            case WindowMessage.WM_DESTROY:
                DestroyWindow(hWnd);
                break;

            case WindowMessage.WM_DPICHANGED:
                if (HasSourceCreated && _hwndSource is not null)
                {
                    _ = SendMessage(_hwndSource.Handle, WindowMessage.WM_DPICHANGED, wParam, lParam);
                    ShowWindow(_hwndSource.Handle, (int)ShowWindowCommands.Show);
                }
                break;

            case WindowMessage.WM_MOUSEWHEEL:
            case WindowMessage.WM_MOUSEHWHEEL:
                // WPF has no event for WM_MOUSEHWHEEL - it never surfaces as a routed input
                // event - so a touchpad's two-finger swipe and a mouse's tilt wheel can only be
                // seen from here. WM_MOUSEWHEEL is taken alongside it because Shift+wheel is the
                // fallback for mice with neither, and both decode through the same place.
                //
                // Not marked handled: a delta that is not a paging gesture (a plain vertical
                // wheel) must still reach WPF, and even one that is should not stop the window's
                // own scrolling if anything inside it ever wants the message.
                {
                    var wheel = WheelDecoder.TryDecode(msg, wParam);
                    if (wheel is int delta) HorizontalWheel?.Invoke(this, delta);
                }
                break;

            case WindowMessage.WM_MOVE:
                // Nothing to do. This used to pin a WS_CHILD HwndSource back to (0,0) inside the
                // container that moved. There is no child any more — WPF's window IS the window —
                // so the same call now drags the OSD to the top-left corner of the screen every
                // time it moves. Observed exactly that way: the notch stopped being centred.
                break;
        }

        // Unhandled messages fall through to WPF, which owns this window now — returning
        // DefWindowProc here would bypass it.
        var result = _hookManager.TryHandleWindowMessage(hWnd, msg, wParam, lParam, out bool hookHandled);
        handled = hookHandled;
        return hookHandled ? result : 0;
    }



    private void UpdateDpiScale(double newDpiScale)
    {
        _dpiScale = newDpiScale;
        UpdateSize(true);
    }

    private void UpdateSize(bool sizeToContent = false)
    {
        if (_isSizeChanging) return;
        _isSizeChanging = true;
        try
        {
            double w = 0, h = 0;
            if (sizeToContent && Content is UIElement content)
            {
                w = content.RenderSize.Width;
                h = content.RenderSize.Height;
            }
            else
            {
                w = ActualWidth;
                h = ActualHeight;
            }
            SetWindowPos(Handle, 0, 0, 0,
                (int)Math.Round(w * _dpiScale),
                (int)Math.Round(h * _dpiScale),
                SWP.NOZORDER | SWP.NOMOVE | SWP.NOACTIVATE);
            UpdateWindow(Handle);
        }
        finally
        {
            _isSizeChanging = false;
        }
    }

    protected void SetPosition(double x, double y)
    {
        if (!HasSourceCreated) return;
        // SetWindowPos takes physical pixels. Callers pass DIPs (WPF-space) since Left/Top
        // are DependencyProperties treated as WPF units elsewhere. Multiplying by _dpiScale
        // matches the size path a few lines up in DpiChangedInternal, which was already
        // scaling width/height into pixels. Without this the OSD lands ~1/dpi of the way
        // across the screen on 125% / 150% / 175% displays instead of at the target corner.
        SetWindowPos(Handle, 0,
            (int)Math.Round(x * _dpiScale), (int)Math.Round(y * _dpiScale),
            0, 0, SWP.NOZORDER | SWP.NOSIZE | SWP.NOACTIVATE);
        UpdateWindow(Handle);
    }

    /// <summary>Current DPI scale factor of the monitor hosting this window (1.0 at 100%,
    /// 1.25 at 125%, ...). Kept in sync by DpiChangedInternal.</summary>
    public double DpiScale => _dpiScale;

    protected virtual void OnSourceCreated() => SourceCreated?.Invoke(this, EventArgs.Empty);

    public void Show()
    {
        if (!HasSourceCreated) CreateWindow();
        if (_isVisibilityChanging) return;
        _isVisibilityChanging = true;
        Visibility = Visibility.Visible;
        _isVisibilityChanging = false;

        // Always show without activating, even when Activatable=true. Activatable governs whether
        // WPF input routing works (NOACTIVATE blocks the WPF child's mouse hit-testing), not
        // whether we proactively steal focus. Showing without activation keeps the user's game
        // or app in the foreground.
        if (TopMost)
        {
            SetWindowPos(Handle, (nint)(-1), 0, 0, 0, 0,
                SWP.NOACTIVATE | SWP.NOMOVE | SWP.NOSIZE | SWP.NOOWNERZORDER | SWP.SHOWWINDOW);
        }
        else
        {
            ShowWindow(Handle, (int)ShowWindowCommands.ShowNoActivate);
        }
        // No child to re-pin: WPF's window IS the window now. The call that used to be here
        // moved it to (0,0), which put the OSD in the top-left corner of the screen on every
        // show instead of at its anchor.
        Shown?.Invoke(this, EventArgs.Empty);
    }

    public void Hide()
    {
        if (!HasSourceCreated || _isVisibilityChanging) return;
        ShowWindow(Handle, (int)ShowWindowCommands.Hide);
        _isVisibilityChanging = true;
        Visibility = Visibility.Hidden;
        _isVisibilityChanging = false;
    }

    public event EventHandler? Activated;
    public event EventHandler? Deactivated;
    public event EventHandler? Shown;
    public event EventHandler? SourceCreated;
}
