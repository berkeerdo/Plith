using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
using System.Windows.Media.Animation;
using Plith.Services;

namespace Plith.Views.Widgets;

/// <summary>
/// A single line of text that scrolls when, and only when, it does not fit.
///
/// Shared between the notch's media widget and the Classic card's media row on purpose: a title
/// must not be written one way on one surface and another way on the other.
///
/// The rules live in <see cref="MarqueeDecision"/>. What lives here is the part that has gone
/// wrong repeatedly on this branch — <b>when</b> to ask. The overflow measurement is re-taken on
/// every size change and every text change and is never cached, because the answer changes when
/// the track changes, when a font falls back to a different face, and when the DPI changes. A
/// value measured once, in a callback that turned out not to fire again, is the shape of every
/// defect this slice is trying not to repeat.
/// </summary>
/// <remarks>
/// A FrameworkElement rather than a Control: it builds its visual tree directly, so a control
/// template would only be a second, empty way to do the same thing — and a Control whose
/// GetVisualChild is overridden while a template is also applied has two competing ideas about
/// what its child is. Foreground and FontSize still arrive by property inheritance, which is how
/// a TextBlock takes them anywhere else in the tree.
/// </remarks>
public sealed class MarqueeText : FrameworkElement
{
    private readonly TextBlock _text = new()
    {
        TextWrapping = TextWrapping.NoWrap,
        HorizontalAlignment = HorizontalAlignment.Left,
        VerticalAlignment = VerticalAlignment.Center,
    };

    private readonly Grid _viewport = new() { ClipToBounds = true };
    private readonly TranslateTransform _shift = new();

    public MarqueeText()
    {
        Focusable = false;
        _text.Foreground = Foreground;
        _text.FontSize = FontSize;
        _text.RenderTransform = _shift;
        _viewport.Children.Add(_text);
        AddVisualChild(_viewport);
        AddLogicalChild(_viewport);

        // Both routes, because they are genuinely different questions: the viewport changing is
        // a layout event, the text changing is a content event, and either alone leaves half the
        // cases un-measured.
        SizeChanged += (_, _) => Resync();

        // The animation must not outlive the control being on screen. In notch mode the OSD's
        // window is never hidden, so a storyboard left running is a permanent cost on an overlay
        // that is invisible most of the time.
        IsVisibleChanged += (_, e) =>
        {
            if ((bool)e.NewValue) Resync();
            else Stop();
        };
        Unloaded += (_, _) => Stop();
    }

    /// <summary>
    /// Real properties rather than inherited ones.
    ///
    /// The first version relied on TextElement.Foreground and friends reaching the inner
    /// TextBlock by inheritance. They did not: the title rendered in the TextBlock's own default
    /// black, which on a near-black card is text that is simply not there. Inheritance through a
    /// FrameworkElement that builds its own visual children is exactly the kind of thing that
    /// works in most trees and quietly does not in one — so nothing here depends on it. Each
    /// property is declared, and each writes straight through to the TextBlock.
    /// </summary>
    public static readonly DependencyProperty ForegroundProperty =
        DependencyProperty.Register(nameof(Foreground), typeof(Brush), typeof(MarqueeText),
            new PropertyMetadata(Brushes.White, (d, e) => Inner(d).Foreground = (Brush)e.NewValue));

    public Brush Foreground
    {
        get => (Brush)GetValue(ForegroundProperty);
        set => SetValue(ForegroundProperty, value);
    }

    public static readonly DependencyProperty FontSizeProperty =
        DependencyProperty.Register(nameof(FontSize), typeof(double), typeof(MarqueeText),
            new FrameworkPropertyMetadata(13.0, FrameworkPropertyMetadataOptions.AffectsMeasure,
                (d, e) => Inner(d).FontSize = (double)e.NewValue));

    public double FontSize
    {
        get => (double)GetValue(FontSizeProperty);
        set => SetValue(FontSizeProperty, value);
    }

    public static readonly DependencyProperty FontWeightProperty =
        DependencyProperty.Register(nameof(FontWeight), typeof(FontWeight), typeof(MarqueeText),
            new FrameworkPropertyMetadata(FontWeights.Normal, FrameworkPropertyMetadataOptions.AffectsMeasure,
                (d, e) => Inner(d).FontWeight = (FontWeight)e.NewValue));

    public FontWeight FontWeight
    {
        get => (FontWeight)GetValue(FontWeightProperty);
        set => SetValue(FontWeightProperty, value);
    }

    public static readonly DependencyProperty FontFamilyProperty =
        DependencyProperty.Register(nameof(FontFamily), typeof(FontFamily), typeof(MarqueeText),
            new FrameworkPropertyMetadata(null, FrameworkPropertyMetadataOptions.AffectsMeasure,
                (d, e) => { if (e.NewValue is FontFamily f) Inner(d).FontFamily = f; }));

    public FontFamily FontFamily
    {
        get => (FontFamily)GetValue(FontFamilyProperty);
        set => SetValue(FontFamilyProperty, value);
    }

    private static TextBlock Inner(DependencyObject d) => ((MarqueeText)d)._text;

    public static readonly DependencyProperty TextProperty =
        DependencyProperty.Register(nameof(Text), typeof(string), typeof(MarqueeText),
            new FrameworkPropertyMetadata(string.Empty, FrameworkPropertyMetadataOptions.AffectsMeasure, OnTextChanged));

    public string Text
    {
        get => (string)GetValue(TextProperty);
        set => SetValue(TextProperty, value);
    }

    private static void OnTextChanged(DependencyObject d, DependencyPropertyChangedEventArgs e)
    {
        if (d is not MarqueeText m) return;
        m._text.Text = e.NewValue as string ?? string.Empty;

        // Back to the start before re-deciding. A new title inheriting the previous one's offset
        // would appear already part-scrolled, with its first characters outside the viewport.
        m.Stop();
        m.Resync();
    }

    protected override int VisualChildrenCount => 1;

    protected override Visual GetVisualChild(int index) => _viewport;

    protected override Size MeasureOverride(Size availableSize)
    {
        // The viewport is measured unconstrained horizontally so the text reports its true
        // width; the constraint is applied on the way out. Measuring it against the available
        // width instead would make the TextBlock report the clipped width, and the overflow this
        // control exists to detect would always be zero.
        _viewport.Measure(new Size(double.PositiveInfinity, availableSize.Height));

        var width = double.IsInfinity(availableSize.Width) ? _viewport.DesiredSize.Width : availableSize.Width;
        return new Size(width, _viewport.DesiredSize.Height);
    }

    protected override Size ArrangeOverride(Size finalSize)
    {
        // Arranged at its full desired width inside a clipped viewport - which is what gives the
        // translation something to reveal. Arranging at finalSize would clip the TextBlock
        // itself, and translating a clipped element moves the hole, not the text.
        _viewport.Arrange(new Rect(0, 0, Math.Max(finalSize.Width, _viewport.DesiredSize.Width), finalSize.Height));
        _viewport.Clip = new RectangleGeometry(new Rect(0, 0, finalSize.Width, finalSize.Height));
        return finalSize;
    }

    /// <summary>
    /// Re-ask the question and act on the answer.
    ///
    /// Reduced motion is read here, at the point of use, rather than captured once: a person can
    /// turn animations off while the app is running, and a control that had cached the answer
    /// would keep scrolling afterwards.
    /// </summary>
    private void Resync()
    {
        var plan = MarqueeDecision.Plan(_text.DesiredSize.Width, ActualWidth, !SystemParameters.ClientAreaAnimation);

        // The fade follows OVERFLOW, not the scroll. Tying it to plan.Scroll was wrong in a way a
        // render showed immediately: that answer is also false when reduced motion is on, so a
        // clipped title got no fade in exactly the case where it will never scroll and the fade
        // is the only thing that can say there is more text.
        var overflows = _text.DesiredSize.Width - ActualWidth > MarqueeDecision.OverflowToleranceDip;
        ApplyEdgeFade(overflows);

        if (!plan.Scroll)
        {
            Stop();
            return;
        }

        // ease-in-out with AutoReverse dwells at both ends for free: the text pauses where the
        // reader starts and pauses again at the end, instead of snapping back under their eye.
        var travel = new DoubleAnimation(0, plan.ShiftDip, plan.Duration)
        {
            BeginTime = StartDelay,
            EasingFunction = new CubicEase { EasingMode = EasingMode.EaseInOut },
            AutoReverse = true,
            RepeatBehavior = RepeatBehavior.Forever,
        };
        _shift.BeginAnimation(TranslateTransform.XProperty, travel);
    }

    /// <summary>
    /// How long the text sits still before it starts, so the beginning is readable before
    /// anything moves.
    ///
    /// Was 1200 ms, and MEASURED against the surface on 2026-09-21: an event-opened panel used to
    /// live about 2.6 s, which left 1.4 s of travel at 28 DIP per second, so a long title moved
    /// about 39 DIP of the 100-plus it needed. The code's own comment claimed a long title
    /// "finishes inside the time an open notch is realistically looked at", and that was false
    /// for the case it described.
    ///
    /// It is less false now for a different reason: since an event gets a HUD rather than the
    /// frame, the frame is only ever opened deliberately and hover keeps it alive, so the window
    /// is as long as someone looks. The delay still comes down, because movement that starts
    /// after more than a second reads as no movement at all to someone who glanced.
    /// </summary>
    private static readonly TimeSpan StartDelay = TimeSpan.FromMilliseconds(700);

    /// <summary>
    /// How wide the softened edge is, in DIP.
    ///
    /// Wide enough that the last glyph fades rather than being cut through, narrow enough not to
    /// dim a word that is fully visible.
    /// </summary>
    private const double FadeWidthDip = 14;

    /// <summary>
    /// Soften the right edge while the text is clipped.
    ///
    /// The mask goes on THIS control rather than on the viewport inside it, and that is the whole
    /// subtlety: the viewport is deliberately arranged at the text's full width and then clipped,
    /// which is what gives the translation something to reveal. An OpacityMask uses its element's
    /// own bounds, so one on the viewport would fade at the end of the TEXT, somewhere off screen,
    /// rather than at the edge where the clip actually happens.
    ///
    /// Only the right edge. The left one could fade too, but the text parks at X = 0 between
    /// legs, and a permanent left fade would dim the first letter of every title that is sitting
    /// still.
    /// </summary>
    private void ApplyEdgeFade(bool clipped)
    {
        if (!clipped || ActualWidth <= FadeWidthDip * 2)
        {
            OpacityMask = null;
            return;
        }

        var solidTo = 1.0 - (FadeWidthDip / ActualWidth);
        var mask = new LinearGradientBrush
        {
            StartPoint = new Point(0, 0.5),
            EndPoint = new Point(1, 0.5),
        };
        mask.GradientStops.Add(new GradientStop(Colors.Black, 0));
        mask.GradientStops.Add(new GradientStop(Colors.Black, solidTo));
        mask.GradientStops.Add(new GradientStop(Colors.Transparent, 1));
        mask.Freeze();
        OpacityMask = mask;
    }

    private void Stop()
    {
        // Clearing the animation before the assignment is not optional: an animated value holds
        // the property with FillBehavior.HoldEnd and outranks a local write, so setting X = 0
        // on its own would silently do nothing and leave the text parked mid-scroll.
        _shift.BeginAnimation(TranslateTransform.XProperty, null);
        _shift.X = 0;
    }
}
