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
        if (Handle == 0 || !HasSourceCreated) return;

        // BOTH windows, not just the container. The WPF content lives in an HwndSource child,
        // and a child is hit-tested in its own right: clearing the bit only on the container
        // left the child capturing every click across the whole rectangle the panel would
        // occupy if it were open — a 440 DIP band across the top of the screen that swallowed
        // clicks while the notch was closed and invisible. Reported on a running build.
        //
        // This was invisible until the container stopped being WS_EX_LAYERED: while it carried
        // that flag with no layered attributes set, the system discarded input before it could
        // reach either window, so the child's own hit-testing never came into play.
        ApplyClickThrough(Handle, isEnabled);
        if (HwndSource is { Handle: var childHandle } && childHandle != 0)
            ApplyClickThrough(childHandle, isEnabled);
    }

    private static void ApplyClickThrough(nint hWnd, bool isEnabled)
    {

        int styles = GetWindowLongPtr(hWnd, (int)GetWindowLongFields.GWL_EXSTYLE).ToInt32();
        // Does NOT re-add WS_EX_LAYERED: see the comment where the window is created. Adding it
        // back here would silently undo that fix on the first click-through toggle, which is the
        // first thing that happens after the window loads.
        int newStyles = styles;
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
