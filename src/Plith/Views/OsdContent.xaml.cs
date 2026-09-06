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
}
