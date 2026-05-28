// Portions adapted from VoicemeeterFancyOSD (MIT, A-tG and contributors). See NOTICE.md.
using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Interop;
using System.Windows.Media;
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
    private readonly WndProc _wndProcDelegate;
    private HwndSource? _hwndSource;
    private double _dpiScale = 1.0;
    private readonly WndProcHookManager _hookManager;
    private bool _isSizeChanging;
    private bool _isVisibilityChanging;

    protected HwndSource? HwndSource => _hwndSource;

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
        _wndProcDelegate = MyWndProc;
        SizeChanged += (_, _) => UpdateSize();
        _hookManager = WndProcHookManager.RegisterForIWndProcObject(this);
        BandWindowExt();
    }

    public void CreateWindow()
    {
        if (HasSourceCreated) return;

        var wndClass = new WNDCLASSEX
        {
            cbSize = Marshal.SizeOf<WNDCLASSEX>(),
            hbrBackground = 0,                               // NULL brush — let the layered window's per-pixel alpha paint everything
            hInstance = Marshal.GetHINSTANCE(typeof(BandWindow).Module),
            lpszMenuName = string.Empty,
            lpszClassName = "PlithBandWindow_" + Guid.NewGuid(),
            lpfnWndProc = Marshal.GetFunctionPointerForDelegate(_wndProcDelegate),
        };
        ushort atom = RegisterClassEx(ref wndClass);
        if (atom == 0) throw new Win32Exception(Marshal.GetLastWin32Error());

        var extStyles = (int)(
            ExtendedWindowStyles.WS_EX_LAYERED |
            ExtendedWindowStyles.WS_EX_NOREDIRECTIONBITMAP |
            // TOOLWINDOW is always wanted: this is an overlay, never a primary app window.
            // Without it, the OSD shows up in the taskbar and Alt+Tab as if it were a real app.
            ExtendedWindowStyles.WS_EX_TOOLWINDOW |
            (IsClickThrough ? ExtendedWindowStyles.WS_EX_TRANSPARENT : 0) |
            (Activatable ? 0 : ExtendedWindowStyles.WS_EX_NOACTIVATE) |
            (TopMost ? ExtendedWindowStyles.WS_EX_TOPMOST : 0));

        // WS_EX_NOREDIRECTIONBITMAP breaks DWM Mica on Win11 — drop it there.
        if (OperatingSystem.IsWindowsVersionAtLeast(10, 0, 22000))
            extStyles &= ~(int)ExtendedWindowStyles.WS_EX_NOREDIRECTIONBITMAP;

        var styles = (uint)WindowStyles.WS_POPUP & ~(uint)WindowStyles.WS_SYSMENU;

        nint hWnd = IsBandWindowSupported()
            ? CreateWindowInBand(extStyles, atom, string.Empty, styles,
                (int)Math.Round(Left), (int)Math.Round(Top), 0, 0,
                0, 0, wndClass.hInstance, 0, (int)ZBandID)
            : CreateWindowEx(extStyles, atom, string.Empty, styles,
                (int)Math.Round(Left), (int)Math.Round(Top), 0, 0,
                0, 0, wndClass.hInstance, 0);

        if (hWnd == 0) throw new Win32Exception(Marshal.GetLastWin32Error());

        Handle = hWnd;
        OnSourceCreated();
        _hookManager.OnHwndCreated(hWnd);

        var param = new HwndSourceParameters
        {
            WindowStyle = (int)(WindowStyles.WS_VISIBLE | WindowStyles.WS_CHILD),
            ParentWindow = hWnd,
            UsesPerPixelTransparency = true,
        };
        _hwndSource = new HwndSource(param)
        {
            SizeToContent = SizeToContent.WidthAndHeight,
            RootVisual = this,
        };
        _hwndSource.CompositionTarget!.BackgroundColor = Colors.Transparent;
        _hwndSource.ContentRendered += (_, _) => UpdateDpiScale(GetDpiForWindow(Handle) / 96.0);
        UpdateWindow(hWnd);
        UpdateDpiScale(GetDpiForWindow(Handle) / 96.0);
        HasSourceCreated = true;
    }

    protected override void OnDpiChanged(DpiScale oldDpi, DpiScale newDpi)
    {
        base.OnDpiChanged(oldDpi, newDpi);
        UpdateDpiScale(newDpi.DpiScaleX);
    }

    private nint MyWndProc(nint hWnd, uint msg, nint wParam, nint lParam)
    {
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

            case WindowMessage.WM_MOVE:
                RepositionHwndSource();
                break;
        }

        var result = _hookManager.TryHandleWindowMessage(hWnd, msg, wParam, lParam, out bool handled);
        return handled ? result : DefWindowProc(hWnd, msg, wParam, lParam);
    }

    private void RepositionHwndSource()
    {
        if (_hwndSource is null) return;
        SetWindowPos(_hwndSource.Handle, 0, 0, 0, 0, 0, SWP.NOSIZE | SWP.NOZORDER | SWP.NOACTIVATE);
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
        SetWindowPos(Handle, 0,
            (int)Math.Round(x), (int)Math.Round(y),
            0, 0, SWP.NOZORDER | SWP.NOSIZE | SWP.NOACTIVATE);
        UpdateWindow(Handle);
    }

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
        RepositionHwndSource();
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
