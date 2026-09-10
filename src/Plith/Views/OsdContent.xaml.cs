using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
using Plith.Views.Presentation;

namespace Plith.Views;

/// <summary>What the notch panel is showing. Exactly one at a time, by construction: which shape
/// the notch takes is what tells a person whether they went somewhere or were answered, and
/// showing two at once would erase that.</summary>
public enum NotchPanelContent
{
    /// <summary>The card stack. Classic's only mode, and never used by the notch.</summary>
    Cards,

    /// <summary>The paged widget frame. Opened by a click.</summary>
    Widgets,

    /// <summary>The short, transient answer to a volume key or a track change.</summary>
    Hud,
}

public partial class OsdContent : UserControl
{
    /// <summary>
    /// The drop-shadow margin on the content root. The card's visible border starts this far
    /// below and inside the content origin, so every geometry calculation that maps between
    /// the window rectangle and what the user actually sees has to subtract it. Exposed as a
    /// constant so no other file repeats the literal.
    /// </summary>
    public const double ContentInsetDip = 14;

    /// <summary>
    /// The card's visible width, and the control's is this plus the insets on both sides.
    ///
    /// Set by the FULLEST row rather than the bare volume one — the same rule the notch's fixed
    /// frame follows. At the previous 412 the layout was generous everywhere and the media row
    /// still ellipsed a real track name; at 300 the media row leaves the title ~130 DIP, which
    /// is where short and medium titles sit still and only genuinely long ones scroll. Windows'
    /// own flyout is wider than either number.
    /// </summary>
    public const double CardWidthDip = 300;

    /// <summary>Compact mode drops the media card entirely, so the fullest row is the volume
    /// one and the card can be narrower. Not proportional to the above: it is what the volume
    /// row needs, measured the same way.</summary>
    public const double CompactCardWidthDip = 224;

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
    /// Show the widget frame instead of the card stack, or the other way round.
    ///
    /// The two are mutually exclusive by construction rather than by convention: a person who
    /// clicked the notch went somewhere, and a person who pressed a volume key was answered, and
    /// the shape is what tells those apart. Showing both would erase the distinction §1 of the
    /// spec exists to make.
    ///
    /// Nothing is re-measured here. Collapsing one and showing the other changes this control's
    /// desired size, and OsdHost's SizeChanged hook picks the new size up after arrange — which
    /// is the only reading that reflects what is actually drawn. A synchronous Measure here would
    /// read the tree before the ItemsControl had materialised its containers, which is the defect
    /// that once drew the ambient row on the panel and the other cards over the bare desktop.
    /// </summary>
    public void SetPanelContent(NotchPanelContent content)
    {
        CardSurface.Visibility = content == NotchPanelContent.Cards ? Visibility.Visible : Visibility.Collapsed;
        WidgetHost.Visibility = content == NotchPanelContent.Widgets ? Visibility.Visible : Visibility.Collapsed;
        HudHost.Visibility = content == NotchPanelContent.Hud ? Visibility.Visible : Visibility.Collapsed;
    }

    /// <summary>The controls the widget frame and the HUD live in. Set once by the host; both
    /// outlive every open and close, so their contents are built once rather than per show —
    /// which matters because those contents own timers and storyboards.</summary>
    /// <summary>
    /// Narrow the card for compact mode, or widen it back.
    ///
    /// Applied to this control rather than to CardSurface, because every geometry calculation in
    /// the notch path maps between the WINDOW rectangle and the visible card through
    /// ContentInsetDip — narrowing the inner Border alone would leave the notch's surface sized
    /// to a card that is no longer that wide.
    /// </summary>
    public void SetCompact(bool compact) =>
        Width = (compact ? CompactCardWidthDip : CardWidthDip) + ContentInsetDip * 2;

    public void SetWidgetContent(UIElement widgets, UIElement hud)
    {
        WidgetHost.Content = widgets;
        HudHost.Content = hud;
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

        // Classic has no widget frame at all. Leaving it visible across a mode switch would put
        // a 356 DIP block inside a floating card that never asked for one.
        if (!notch) SetPanelContent(NotchPanelContent.Cards);

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
