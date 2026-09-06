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

        // Deliberately NOT followed by SetLayeredWindowAttributes(hWnd, 0, 255, LWA_ALPHA).
        //
        // That call used to live here, and it is destructive on this window: the HwndSource is
        // created with UsesPerPixelTransparency = true, so the surface carries a real alpha
        // channel. SetLayeredWindowAttributes switches a layered window to CONSTANT-alpha
        // layering, which discards the per-pixel channel — every transparent pixel turns
        // opaque black. In notch mode the window is never hidden, so the result is a permanent
        // black rectangle sitting at the top of the screen rather than a transient glitch.
        //
        // It was harmless only because nothing ever reached it: IsClickThrough was assigned
        // once in OsdHost's constructor, before CreateWindow(), when the setter still returns
        // early on !HasSourceCreated. Re-asserting click-through after Loaded (needed so the
        // parked notch does not swallow clicks) made this the first code path in Plith to
        // actually run it, and it broke rendering on the first launch that did.
        //
        // Nothing here needs it: WS_EX_LAYERED is already applied at window creation, and
        // click-through is entirely a matter of the WS_EX_TRANSPARENT bit set above.
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
