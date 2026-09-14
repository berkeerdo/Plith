using System.Windows;
using System.Windows.Automation;
using System.Windows.Controls;
using System.Windows.Controls.Primitives;
using System.Windows.Media;
using System.Windows.Media.Animation;
using System.Windows.Shapes;
using Plith.Services;
using Plith.Views.Presentation;

namespace Plith.Views.Widgets;

/// <summary>
/// The open notch's widget surface: one fixed frame, pages moving through it, and a lane of
/// page dots that the pages can never displace.
///
/// The frame owns no page state. <see cref="NotchPager"/> owns the index, and this control asks
/// it — the index is one of the three values in this slice that sit squarely in the defect class
/// this branch keeps producing (state derived once, in a callback that turned out not to fire),
/// and an index living in an object with no animation and no completion handler cannot go stale.
/// </summary>
public partial class WidgetFrame : UserControl
{
    private const int SlideMs = 260;

    private readonly List<FrameworkElement> _pages = new();
    private readonly List<ToggleButton> _dots = new();

    /// <summary>
    /// The page currently on its way out, if any.
    ///
    /// Kept as a field rather than being removed only by the slide's completion handler. WPF
    /// raises no Completed for a clock a competing animation replaced, so a page turned twice in
    /// quick succession would leave the first outgoing page in the tree forever, stacked behind
    /// the live one. Both routes remove it: whichever happens first wins, and the other finds
    /// nothing to do.
    /// </summary>
    private FrameworkElement? _outgoing;

    public WidgetFrame()
    {
        InitializeComponent();
        Width = NotchGeometry.OpenFrameDip.Width;
        Height = NotchGeometry.OpenFrameDip.Height;
        // Set in XAML now: the lane sits over the page rather than under it, so its spacing is
        // a bottom inset rather than a gap above.
    }

    /// <summary>The pager this frame reads its index from. Set once by the host.</summary>
    public NotchPager? Pager { get; private set; }

    /// <summary>Raised when a page dot is clicked, carrying the page it asked for. The host
    /// drives the pager rather than the frame doing it, so there is exactly one place that
    /// decides what a page change means.</summary>
    public event EventHandler<int>? PageRequested;

    /// <summary>
    /// Install the pages. Order is the paging order.
    ///
    /// Replaces whatever was there: pages come and go (the media page only exists while
    /// something is playing), and the pager is told the new count so an index pointing past the
    /// end is brought back into range rather than throwing on the next render.
    /// </summary>
    public void SetPages(NotchPager pager, IReadOnlyList<FrameworkElement> pages)
    {
        ArgumentNullException.ThrowIfNull(pager);
        ArgumentNullException.ThrowIfNull(pages);
        if (pages.Count == 0) throw new ArgumentException("A frame needs at least one page.", nameof(pages));

        Pager = pager;
        _pages.Clear();
        _pages.AddRange(pages);
        pager.SetPageCount(_pages.Count);

        PageHost.Children.Clear();
        _outgoing = null;
        BuildDots();
        ShowCurrentPage(direction: 0, animate: false);
    }

    /// <summary>
    /// Bring the frame into line with the pager's index.
    ///
    /// <paramref name="direction"/> is the sign of the page change, so the incoming page enters
    /// from the side the gesture came from. Zero means no slide — used when the frame is first
    /// populated, and when reduced motion is in force.
    /// </summary>
    public void SyncToPager(int direction)
    {
        if (Pager is null) return;
        ShowCurrentPage(direction, animate: direction != 0 && IsAnimationAllowed);
    }

    /// <summary>
    /// Whether page transitions may animate.
    ///
    /// Read at the point of use, never captured at construction: a person can turn animations
    /// off while the app is running, and a value cached in a constructor would keep sliding
    /// afterwards.
    /// </summary>
    private static bool IsAnimationAllowed => SystemParameters.ClientAreaAnimation;

    private void ShowCurrentPage(int direction, bool animate)
    {
        if (Pager is null || _pages.Count == 0) return;

        var index = Math.Clamp(Pager.Index, 0, _pages.Count - 1);
        var incoming = _pages[index];
        var current = PageHost.Children.Count > 0 ? PageHost.Children[^1] as FrameworkElement : null;
        if (ReferenceEquals(current, incoming))
        {
            UpdateDots(index);
            return;
        }

        // Any page still on its way out from a previous turn goes now. See the field's comment:
        // relying on the earlier animation's Completed to do it is the failure mode.
        RemoveOutgoing();

        if (current is not null)
        {
            if (animate) SlideOut(current, direction);
            else PageHost.Children.Remove(current);
        }

        PageHost.Children.Add(incoming);
        incoming.Opacity = 1;
        incoming.RenderTransform = null;
        if (animate) SlideIn(incoming, direction);

        UpdateDots(index);
    }

    private static void SlideIn(FrameworkElement page, int direction)
    {
        var from = direction >= 0 ? NotchGeometry.PageSlideDip : -NotchGeometry.PageSlideDip;
        var shift = new TranslateTransform(from, 0);
        page.RenderTransform = shift;
        page.Opacity = 0;

        var ease = new CubicEase { EasingMode = EasingMode.EaseOut };
        var duration = TimeSpan.FromMilliseconds(SlideMs);
        shift.BeginAnimation(TranslateTransform.XProperty,
            new DoubleAnimation(0, duration) { EasingFunction = ease });
        page.BeginAnimation(OpacityProperty,
            new DoubleAnimation(1, duration) { EasingFunction = ease });
    }

    private void SlideOut(FrameworkElement page, int direction)
    {
        var to = direction >= 0 ? -NotchGeometry.PageSlideDip : NotchGeometry.PageSlideDip;
        var shift = new TranslateTransform();
        page.RenderTransform = shift;
        _outgoing = page;

        var duration = TimeSpan.FromMilliseconds(SlideMs);
        var ease = new CubicEase { EasingMode = EasingMode.EaseOut };
        shift.BeginAnimation(TranslateTransform.XProperty,
            new DoubleAnimation(to, duration) { EasingFunction = ease });

        var fade = new DoubleAnimation(0, duration) { EasingFunction = ease };
        fade.Completed += (_, _) =>
        {
            // Guarded, because this page may already have been removed by the next page change.
            // Removing a child twice is harmless, but re-checking is what makes the two routes
            // independent rather than merely ordered.
            if (ReferenceEquals(_outgoing, page)) RemoveOutgoing();
        };
        page.BeginAnimation(OpacityProperty, fade);
    }

    private void RemoveOutgoing()
    {
        if (_outgoing is null) return;

        // The animations are cleared before the element leaves, so a held animated value cannot
        // outlive it: this element is reused the next time its page comes round, and a page that
        // arrived still holding an old fade would appear at whatever opacity the fade had
        // reached. FillBehavior.HoldEnd outranks a local write, so assigning Opacity = 1 without
        // this would silently do nothing.
        _outgoing.BeginAnimation(OpacityProperty, null);
        if (_outgoing.RenderTransform is TranslateTransform t)
            t.BeginAnimation(TranslateTransform.XProperty, null);
        _outgoing.RenderTransform = null;
        _outgoing.Opacity = 1;

        PageHost.Children.Remove(_outgoing);
        _outgoing = null;
    }

    private void BuildDots()
    {
        foreach (var dot in _dots) dot.Click -= OnDotClicked;
        _dots.Clear();
        Dots.Children.Clear();

        // One dot alone says nothing except that there is nowhere to go, so it is not drawn.
        if (_pages.Count < 2) return;

        for (int i = 0; i < _pages.Count; i++)
        {
            var dot = new ToggleButton
            {
                // 20 wide so a click between two dots still lands on one; 14 tall because that
                // is the whole lane, and a taller target would reach up into the page and take
                // clicks meant for a transport button or a volume track.
                Width = 20,
                Height = NotchGeometry.DotsLaneDip,
                Padding = new Thickness(0),
                BorderThickness = new Thickness(0),
                Background = Brushes.Transparent,
                Cursor = System.Windows.Input.Cursors.Hand,
                Focusable = true,
                Tag = i,
                Template = BuildDotTemplate(),
            };

            // On the ToggleButton itself, which has a peer. A name on the Ellipse inside the
            // template would reach nothing.
            AutomationProperties.SetName(dot, $"Page {i + 1} of {_pages.Count}");
            dot.Click += OnDotClicked;

            _dots.Add(dot);
            Dots.Children.Add(dot);
        }
    }

    /// <summary>A dot is a 5 DIP circle centred in a 20 x 14 hit area. Built here rather than in
    /// a resource dictionary so the two sizes stay next to the comment explaining them.</summary>
    private static ControlTemplate BuildDotTemplate()
    {
        var ellipse = new FrameworkElementFactory(typeof(Ellipse));
        ellipse.SetValue(WidthProperty, 5.0);
        ellipse.SetValue(HeightProperty, 5.0);
        ellipse.SetValue(HorizontalAlignmentProperty, HorizontalAlignment.Center);
        ellipse.SetValue(VerticalAlignmentProperty, VerticalAlignment.Center);
        ellipse.SetValue(Shape.FillProperty, new TemplateBindingExtension(ForegroundProperty));

        var root = new FrameworkElementFactory(typeof(Border));
        root.SetValue(BackgroundProperty, Brushes.Transparent);
        root.AppendChild(ellipse);

        return new ControlTemplate(typeof(ToggleButton)) { VisualTree = root };
    }

    private void UpdateDots(int index)
    {
        for (int i = 0; i < _dots.Count; i++)
        {
            var on = i == index;
            _dots[i].IsChecked = on;
            _dots[i].Foreground = on ? ActiveDotBrush : IdleDotBrush;
        }
    }

    private static readonly Brush ActiveDotBrush = Freeze(Color.FromRgb(0xC6, 0xD0, 0xDA));
    private static readonly Brush IdleDotBrush = Freeze(Color.FromRgb(0x46, 0x52, 0x5F));

    private static SolidColorBrush Freeze(Color color)
    {
        var brush = new SolidColorBrush(color);
        brush.Freeze();
        return brush;
    }

    private void OnDotClicked(object sender, RoutedEventArgs e)
    {
        if (sender is ToggleButton { Tag: int index }) PageRequested?.Invoke(this, index);
    }
}
