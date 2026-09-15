using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
using System.Windows.Media.Animation;
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
    /// <remarks>
    /// Back to what shipped in 0.1.5. It was briefly 300, taken from the mockup — but that
    /// mockup is a card drawn inside a web page, where 300 sits in context next to body text,
    /// and the real OSD sits alone on a large screen with nothing to be judged against. Reported
    /// as too small twice. The old width was never a complaint; narrowing it created one.
    /// </remarks>
    public const double CardWidthDip = 412;

    /// <summary>Compact mode drops the media card entirely, so the fullest row is the volume
    /// one and the card can be narrower. Not proportional to the above: it is what the volume
    /// row needs, measured the same way.</summary>
    public const double CompactCardWidthDip = 300;

    /// <summary>Peak opacity of the notch panel's drop shadow, reached only when fully open.</summary>
    private const double NotchShadowOpacity = 0.5;

    /// <summary>How far through a change of open size, 0..1. A dependency property so one
    /// DoubleAnimation drives it, for the same reason NotchExpand is one.</summary>
    public static readonly DependencyProperty MorphProperty =
        DependencyProperty.Register(
            nameof(Morph), typeof(double), typeof(OsdContent),
            new PropertyMetadata(1.0, (d, e) =>
            {
                if (d is OsdContent c) c.ApplyNotchExpand(c.NotchExpand);
            }));

    public double Morph
    {
        get => (double)GetValue(MorphProperty);
        set => SetValue(MorphProperty, value);
    }

    public static readonly DependencyProperty NotchExpandProperty =
        DependencyProperty.Register(
            nameof(NotchExpand), typeof(double), typeof(OsdContent),
            new PropertyMetadata(0.0, OnNotchExpandChanged));

    private bool _notchLook;
    private double _collapsedHeight;

    /// <summary>
    /// The open size the shape is morphing FROM and TO, and how far along that is.
    ///
    /// The open size used to be one field, assigned outright. That is correct while the notch is
    /// closed — it opens into whatever the content now measures — and wrong while it is already
    /// open, because the panel can CHANGE size without closing: a volume HUD is 300 x 46, a
    /// media HUD 372 x 54, the widget frame 356 x 116. Clicking a HUD to open the frame snapped
    /// the shape between two of those in a single frame, which is the hard jump reported.
    ///
    /// Two sizes and a progress value make that a morph instead. Everything is still derived
    /// from NotchExpand as before; only the far end of the interpolation now moves.
    /// </summary>
    private Size _expandedFrom;
    private Size _expandedTo;

    /// <summary>Whether the panel area is currently held at the larger of two sizes. Read by
    /// SetNotchMetrics so it can tell a real content change from its own pin.</summary>
    private bool _panelPinned;

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
        // A measurement taken while the panel area is pinned is the PIN talking, not new
        // content — and acting on it is a feedback loop: the pin makes the control measure
        // large, the large measurement starts a morph back to large, and the panel never
        // shrinks at all. Which is exactly what the first version of this pin did.
        if (_panelPinned) return;

        var target = new Size(
            Math.Max(0, measuredContentSize.Width - ContentInsetDip * 2),
            Math.Max(0, measuredContentSize.Height - ContentInsetDip));

        if (SizesMatch(target, _expandedTo)) return;

        // Closed, or barely open: take the new size outright. There is nothing on screen big
        // enough for a morph to be visible on, and animating here would make the notch's own
        // opening start from whatever the last panel happened to be.
        var openEnough = _notchLook && NotchExpand > 0.5;

        _expandedFrom = openEnough ? CurrentExpanded() : target;
        _expandedTo = target;

        BeginAnimation(MorphProperty, null);
        if (!openEnough)
        {
            Morph = 1;
            ApplyNotchExpand(NotchExpand);
            return;
        }

        // Hold the panel area at the LARGER of the two sizes for the length of the morph.
        //
        // The window sizes itself to this control, so a panel that shrinks in one frame takes
        // the window with it - and the surface's morph then plays out inside a window that has
        // already finished. That is the shrink that looked comical: a shape animating inside a
        // box that had snapped. Pinned to the maximum, the window stays big enough and the black
        // shape carries the whole animation; the extra area is transparent, so releasing the pin
        // at the end shows nothing going away.
        SlidingRoot.MinWidth = Math.Max(_expandedFrom.Width, _expandedTo.Width);
        SlidingRoot.MinHeight = Math.Max(_expandedFrom.Height, _expandedTo.Height);
        _panelPinned = true;

        Morph = 0;
        BeginAnimation(MorphProperty, new DoubleAnimation(1.0, TimeSpan.FromMilliseconds(MorphMs))
        {
            // Out only, and on the same curve the panel's own expansion uses, so a shape that
            // grows while opening and a shape that grows while already open move alike.
            EasingFunction = new CubicEase { EasingMode = EasingMode.EaseOut },
        });
    }

    /// <summary>How long one panel takes to become another. Shorter than the notch's own
    /// opening: the shape is already on screen and only changing size, and a change that takes
    /// as long as an arrival reads as the panel re-opening.</summary>
    private const int MorphMs = 260;

    /// <summary>The open size right now, part-way between the two ends of a morph.</summary>
    private Size CurrentExpanded()
    {
        var t = NotchGeometry.Clamp01(Morph);
        return new Size(
            NotchGeometry.Lerp(_expandedFrom.Width, _expandedTo.Width, t),
            NotchGeometry.Lerp(_expandedFrom.Height, _expandedTo.Height, t));
    }

    private static bool SizesMatch(Size a, Size b) =>
        Math.Abs(a.Width - b.Width) < 0.5 && Math.Abs(a.Height - b.Height) < 0.5;

    private void ApplyNotchExpand(double t)
    {
        // Classic never morphs: its card is CardSurface, laid out and faded by the window.
        // Guarding here rather than at every call site means a stray write to NotchExpand
        // while Classic is active cannot resize a surface Classic does not even show.
        if (!_notchLook) return;

        var size = NotchGeometry.SurfaceSize(
            NotchGeometry.CollapsedWidthDip, _collapsedHeight, CurrentExpanded(), t);

        NotchSurface.Width = size.Width;
        NotchSurface.Height = size.Height;

        var radius = NotchGeometry.SurfaceRadius(t, size.Height);
        NotchSurface.CornerRadius = new CornerRadius(0, 0, radius, radius);
        NotchShadow.Opacity = NotchGeometry.Lerp(0, NotchShadowOpacity, t);

        SlidingRoot.Opacity = NotchGeometry.ContentOpacity(t);

        // Released here rather than in the morph's completion handler. A clock replaced by a
        // competing animation raises no Completed - the hazard behind more defects on this
        // branch than any other - and a pin left on would freeze the panel at the largest size
        // it ever had. Derived from the value itself, it cannot be missed.
        if (Morph >= 0.999 && _panelPinned)
        {
            _panelPinned = false;
            SlidingRoot.MinWidth = 0;
            SlidingRoot.MinHeight = 0;
        }
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
    /// <summary>What the panel is showing right now. Read where it is needed rather than
    /// mirrored by callers, which is how the rest of this branch learned to keep state.</summary>
    public NotchPanelContent PanelContent { get; private set; } = NotchPanelContent.Cards;

    public void SetPanelContent(NotchPanelContent content)
    {
        if (PanelContent == content) return;

        var wasOpen = _notchLook && NotchExpand > 0.5;
        PanelContent = content;

        CardSurface.Visibility = content == NotchPanelContent.Cards ? Visibility.Visible : Visibility.Collapsed;
        WidgetHost.Visibility = content == NotchPanelContent.Widgets ? Visibility.Visible : Visibility.Collapsed;
        HudHost.Visibility = content == NotchPanelContent.Hud ? Visibility.Visible : Visibility.Collapsed;

        if (!wasOpen) return;

        // The shape is already morphing to the new panel's size, so the panel arrives INTO it
        // rather than being there the instant the swap happens. Only the incoming half fades:
        // the outgoing one is collapsed outright, because holding two panels on screen to cross
        // them would need a completion callback to take the old one away, and a callback that
        // does not fire is the defect this branch has produced more than any other.
        var incoming = Incoming(content);
        if (incoming is null) return;

        incoming.BeginAnimation(OpacityProperty, null);
        incoming.Opacity = 0;
        incoming.BeginAnimation(OpacityProperty,
            new DoubleAnimation(1, TimeSpan.FromMilliseconds(PanelFadeMs))
            {
                // Begins a little into the morph, so the shape is closer to its new size before
                // anything is legible in it - the same ordering the notch's own opening uses.
                BeginTime = TimeSpan.FromMilliseconds(70),
                EasingFunction = new CubicEase { EasingMode = EasingMode.EaseOut },
            });
    }

    private const int PanelFadeMs = 180;

    private UIElement? Incoming(NotchPanelContent content) => content switch
    {
        NotchPanelContent.Cards => CardSurface,
        NotchPanelContent.Widgets => WidgetHost,
        NotchPanelContent.Hud => HudHost,
        _ => null,
    };

    /// <summary>The controls the widget frame and the HUD live in. Set once by the host; both
    /// outlive every open and close, so their contents are built once rather than per show —
    /// which matters because those contents own timers and storyboards.</summary>
    /// <summary>
    /// Set the shell's width for the presentation and mode it is in.
    ///
    /// Applied to this control rather than to CardSurface, because every geometry calculation in
    /// the notch path maps between the WINDOW rectangle and the visible card through
    /// ContentInsetDip — narrowing the inner Border alone would leave the notch's surface sized
    /// to a card that is no longer that wide.
    ///
    /// <b>The notch does not use the card's width, and assuming it did was a real regression.</b>
    /// Sizing the card by its fullest row took this control from 440 to 328, which is narrower
    /// than the widget frame (356) and narrower still than the wide media HUD (372) — so the
    /// notch's own panel was being clipped by the shell around it, and its content had nowhere
    /// to lay out. In notch mode the width therefore comes from the widest shape the NOTCH can
    /// show, not from the card, and the two no longer share a number they never shared a meaning
    /// with.
    /// </summary>
    public void SetShellWidth(bool notch, bool compact)
    {
        if (notch)
        {
            // Auto, not a fixed number. The notch shows three shapes of three widths - the
            // widget frame at 356, the volume HUD at 300, the media HUD at 372 - and the
            // surface behind them is derived from THIS control's measured size. Pinning it to
            // the widest left the black shape 372 across whatever was actually showing, with
            // the panel floating inside it. Sizing to content makes the shape hug the panel,
            // which is the whole idea of a notch that grows into what it is showing.
            Width = double.NaN;
            return;
        }

        Width = (compact ? CompactCardWidthDip : CardWidthDip) + ContentInsetDip * 2;
    }

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
