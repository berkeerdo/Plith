using System.Windows;
using System.Windows.Controls;

namespace Plith.Views;

public partial class OsdContent : UserControl
{
    /// <summary>
    /// The drop-shadow margin on the sliding root. The card's visible border starts this far
    /// below and inside the content origin, so every geometry calculation that maps between
    /// the window rectangle and what the user actually sees has to subtract it. Exposed as a
    /// constant so no other file repeats the literal.
    /// </summary>
    public const double ContentInsetDip = 14;

    public static readonly DependencyProperty ContentOffsetProperty =
        DependencyProperty.Register(
            nameof(ContentOffset), typeof(double), typeof(OsdContent),
            new PropertyMetadata(0.0, OnContentOffsetChanged));

    public OsdContent() => InitializeComponent();

    /// <summary>Vertical offset of the card, in DIP. Negative parks it above the top edge.
    /// A dependency property so it can be the target of a DoubleAnimation.</summary>
    public double ContentOffset
    {
        get => (double)GetValue(ContentOffsetProperty);
        set => SetValue(ContentOffsetProperty, value);
    }

    private static void OnContentOffsetChanged(DependencyObject d, DependencyPropertyChangedEventArgs e)
    {
        if (d is OsdContent c) c.ContentSlide.Y = (double)e.NewValue;
    }

    /// <summary>Show or hide the parked strip. Height follows the user's setting.</summary>
    public void SetStrip(bool visible, double heightDip)
    {
        NotchStrip.Height = heightDip;
        NotchStrip.Visibility = visible ? Visibility.Visible : Visibility.Collapsed;
    }

    /// <summary>
    /// Switch the card between the Classic floating-card shape and the notch shape.
    ///
    /// The notch is attached to the top edge of the screen, so it drops the top margin, squares
    /// its top corners, removes its top border and casts its shadow only downward. Without that
    /// it is a rounded card that merely sits near the top — the strip and the descended panel
    /// read as two separate objects with a gap between them, which is what "it doesn't behave
    /// like a notch" describes.
    ///
    /// The vertical geometry stays valid across the switch because only the TOP margin changes.
    /// <see cref="ContentInsetDip"/> is still the distance from the content's bottom edge to the
    /// card's, which is the only inset <c>NotchGeometry.HiddenOffset</c> uses, and it is still
    /// the horizontal inset that <c>NotchGeometry.StripRect</c> uses. Changing the bottom or
    /// side margins here would silently break both.
    /// </summary>
    public void SetNotchLook(bool notch)
    {
        SlidingRoot.Margin = notch
            ? new Thickness(ContentInsetDip, 0, ContentInsetDip, ContentInsetDip)
            : new Thickness(ContentInsetDip);

        CardSurface.CornerRadius = notch ? new CornerRadius(0, 0, 14, 14) : new CornerRadius(14);
        CardSurface.BorderThickness = notch ? new Thickness(1, 0, 1, 1) : new Thickness(1);

        // ShadowDepth pushes the blur downward; Direction 270 is straight down in WPF's
        // clockwise-from-east convention. Classic keeps ShadowDepth 0, which spreads the blur
        // evenly in every direction.
        CardShadow.ShadowDepth = notch ? 6 : 0;
        CardShadow.Direction = notch ? 270 : 0;
    }
}
