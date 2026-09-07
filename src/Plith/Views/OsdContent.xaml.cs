using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
using Plith.Views.Presentation;

namespace Plith.Views;

public partial class OsdContent : UserControl
{
    /// <summary>
    /// The drop-shadow margin on the content root. The card's visible border starts this far
    /// below and inside the content origin, so every geometry calculation that maps between
    /// the window rectangle and what the user actually sees has to subtract it. Exposed as a
    /// constant so no other file repeats the literal.
    /// </summary>
    public const double ContentInsetDip = 14;

    /// <summary>Peak opacity of the notch panel's drop shadow, reached only when fully open.</summary>
    private const double NotchShadowOpacity = 0.5;

    public static readonly DependencyProperty NotchExpandProperty =
        DependencyProperty.Register(
            nameof(NotchExpand), typeof(double), typeof(OsdContent),
            new PropertyMetadata(0.0, OnNotchExpandChanged));

    private bool _notchLook;
    private double _collapsedHeight;
    private Size _expandedSize;

    public OsdContent() => InitializeComponent();

    /// <summary>
    /// How far the notch is open: 0 is the collapsed pill, 1 the fully open panel.
    ///
    /// A dependency property so a single DoubleAnimation can drive it. Everything the
    /// expansion touches — surface width, surface height, corner radius, shadow strength and
    /// content opacity — is derived from this one value in <see cref="ApplyNotchExpand"/>,
    /// rather than being animated in parallel. Parallel clocks on the same visual transition
    /// can be stopped, retargeted or completed independently, and any of those leaves the
    /// shape in a state no single progress value describes.
    /// </summary>
    public double NotchExpand
    {
        get => (double)GetValue(NotchExpandProperty);
        set => SetValue(NotchExpandProperty, value);
    }

    private static void OnNotchExpandChanged(DependencyObject d, DependencyPropertyChangedEventArgs e)
    {
        if (d is OsdContent c) c.ApplyNotchExpand((double)e.NewValue);
    }

    /// <summary>
    /// The two numbers the morph needs from outside: the user's resting height and the
    /// measured size of the card content. Re-applies the current expansion, so a media card
    /// appearing or a settings change is picked up immediately instead of at the next
    /// animation — while parked, no animation is coming.
    /// </summary>
    public void SetNotchMetrics(double collapsedHeightDip, Size measuredContentSize)
    {
        _collapsedHeight = Math.Max(0, collapsedHeightDip);

        // Convert the UserControl's measured size into the visible surface rectangle. The
        // content root is inset by ContentInsetDip on the left, right and bottom in notch mode
        // (SetNotchLook drops the top inset so the panel is flush with the screen edge), so the
        // surface that must coincide with it at full expansion is that much smaller.
        _expandedSize = new Size(
            Math.Max(0, measuredContentSize.Width - ContentInsetDip * 2),
            Math.Max(0, measuredContentSize.Height - ContentInsetDip));

        ApplyNotchExpand(NotchExpand);
    }

    private void ApplyNotchExpand(double t)
    {
        // Classic never morphs: its card is CardSurface, laid out and faded by the window.
        // Guarding here rather than at every call site means a stray write to NotchExpand
        // while Classic is active cannot resize a surface Classic does not even show.
        if (!_notchLook) return;

        var size = NotchGeometry.SurfaceSize(
            NotchGeometry.CollapsedWidthDip, _collapsedHeight, _expandedSize, t);

        NotchSurface.Width = size.Width;
        NotchSurface.Height = size.Height;

        var radius = NotchGeometry.SurfaceRadius(t, size.Height);
        NotchSurface.CornerRadius = new CornerRadius(0, 0, radius, radius);
        NotchShadow.Opacity = NotchGeometry.Lerp(0, NotchShadowOpacity, t);

        SlidingRoot.Opacity = NotchGeometry.ContentOpacity(t);
    }

    /// <summary>
    /// Switch the card between the Classic floating-card shape and the notch shape.
    ///
    /// The notch is attached to the top edge of the screen, so the content root drops its top
    /// margin and CardSurface stops painting entirely — background, border and shadow all come
    /// from NotchSurface behind it, which is the shape that actually morphs. Letting CardSurface
    /// keep painting in notch mode would draw a second outline and a second shadow on top of
    /// the panel, one DIP out of register with it.
    ///
    /// The horizontal geometry stays valid across the switch because the left and right insets
    /// never change: <see cref="ContentInsetDip"/> is still the distance from the content's
    /// side edges to the card's, which is what <c>SetNotchMetrics</c> subtracts to get the
    /// surface width. Changing the side margins here would silently break the alignment
    /// between the two elements at full expansion.
    /// </summary>
    public void SetNotchLook(bool notch)
    {
        _notchLook = notch;

        SlidingRoot.Margin = notch
            ? new Thickness(ContentInsetDip, 0, ContentInsetDip, ContentInsetDip)
            : new Thickness(ContentInsetDip);

        NotchSurface.Visibility = notch ? Visibility.Visible : Visibility.Collapsed;

        CardSurface.Background = notch ? Brushes.Transparent : FindResource("OsdSurfaceBrush") as Brush;
        CardSurface.BorderThickness = notch ? new Thickness(0) : new Thickness(1);
        CardSurface.CornerRadius = notch ? new CornerRadius(0) : new CornerRadius(14);

        // ShadowDepth 0 spreads the blur evenly in every direction, which is Classic's look.
        // Direction is restored to DropShadowEffect's own default of 315 rather than 0: the
        // XAML never declares it, so 315 is what a Classic-only run has always had, and 0 (due
        // east) would silently become Classic's shadow angle the moment anyone gives it a
        // non-zero ShadowDepth. Inert today, wrong the instant it is not.
        CardShadow.Opacity = notch ? 0 : 0.55;
        CardShadow.ShadowDepth = 0;
        CardShadow.Direction = 315;

        // Classic does its fading with the window's Opacity and never touches NotchExpand, so
        // it needs the content root left fully opaque. Without this a mode switch away from a
        // collapsed notch would hand Classic a card at opacity 0.
        if (!notch) SlidingRoot.Opacity = 1;
        else ApplyNotchExpand(NotchExpand);
    }
}
