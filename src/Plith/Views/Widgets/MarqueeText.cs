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

    /// <summary>How long the text sits still before it starts, so the beginning is readable
    /// before anything moves.</summary>
    private static readonly TimeSpan StartDelay = TimeSpan.FromMilliseconds(1200);

    private void Stop()
    {
        // Clearing the animation before the assignment is not optional: an animated value holds
        // the property with FillBehavior.HoldEnd and outranks a local write, so setting X = 0
        // on its own would silently do nothing and leave the text parked mid-scroll.
        _shift.BeginAnimation(TranslateTransform.XProperty, null);
        _shift.X = 0;
    }
}
