// Portions adapted from VoicemeeterFancyOSD (MIT, A-tG and contributors). See NOTICE.md.
using System.Windows;
using static Plith.Interop.NativeMethods;

namespace Plith.Interop;

public partial class BandWindow
{
    private void BandWindowExt()
    {
        Loaded += InitCustomProperties;
        Application.Current.Exit += OnAppExit;
    }

    public static readonly DependencyProperty LeftProperty =
        DependencyProperty.Register(nameof(Left), typeof(double), typeof(BandWindow), new PropertyMetadata(0.0));
    public double Left
    {
        get => (double)GetValue(LeftProperty);
        set { SetPosition(value, Top); SetValue(LeftProperty, value); }
    }

    public static readonly DependencyProperty TopProperty =
        DependencyProperty.Register(nameof(Top), typeof(double), typeof(BandWindow), new PropertyMetadata(0.0));
    public double Top
    {
        get => (double)GetValue(TopProperty);
        set { SetPosition(Left, value); SetValue(TopProperty, value); }
    }

    public static readonly DependencyProperty IsClickThroughProperty =
        DependencyProperty.Register(nameof(IsClickThrough), typeof(bool), typeof(BandWindow),
            new PropertyMetadata(true));
    public bool IsClickThrough
    {
        get => (bool)GetValue(IsClickThroughProperty);
        set
        {
            SetValue(IsClickThroughProperty, value);
            if (!IsLoaded || !HasSourceCreated) return;
            ToggleClickThrough(value);
        }
    }

    private void ToggleClickThrough(bool isEnabled)
    {
        var hWnd = Handle;
        if (hWnd == 0 || !HasSourceCreated) return;

        int styles = GetWindowLongPtr(hWnd, (int)GetWindowLongFields.GWL_EXSTYLE).ToInt32();
        int newStyles = styles | (int)ExtendedWindowStyles.WS_EX_LAYERED;
        if (isEnabled) newStyles |= (int)ExtendedWindowStyles.WS_EX_TRANSPARENT;
        else newStyles &= ~(int)ExtendedWindowStyles.WS_EX_TRANSPARENT;
        if (styles == newStyles) return;

        SetWindowLongPtr(hWnd, (int)GetWindowLongFields.GWL_EXSTYLE, newStyles);
        SetLayeredWindowAttributes(hWnd, 0, 255, LWA_ALPHA);
    }

    private void InitCustomProperties(object sender, RoutedEventArgs e)
    {
        SetPosition(Left, Top);
    }

    private void OnAppExit(object? sender, EventArgs e)
    {
        // Avoid "Invalid window handle" on shutdown by disposing the HwndSource explicitly.
        try { HwndSource?.Dispose(); } catch { }
    }
}
