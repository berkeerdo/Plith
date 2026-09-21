using System.Windows;
using System.Windows.Automation;
using System.Windows.Controls;
using System.Windows.Controls.Primitives;
using System.Windows.Media;
using System.Windows.Media.Animation;
using System.Windows.Shapes;
using System.Windows.Threading;
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
    /// <summary>Taken from NotchGeometry, where the catcher can read it too: the shelf page is
    /// drawn by the other process and has to arrive on the same curve over the same time, or the
    /// page turn onto it reads as a different kind of movement. See PageSlideMs.</summary>
    private const int SlideMs = NotchGeometry.PageSlideMs;

    private readonly List<FrameworkElement> _pages = new();

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
        WireRail();
        Width = NotchGeometry.OpenFrameDip.Width;
        Height = NotchGeometry.OpenFrameDip.Height;
        // Set in XAML now: the lane sits over the page rather than under it, so its spacing is
        // a bottom inset rather than a gap above.
    }

    /// <summary>
    /// The page on screen, or null before any is installed.
    ///
    /// Exposed so the host can ask what is being looked at. It matters for one decision: an
    /// event that describes the page you are already on should not replace that page with a
    /// two-second notice about it, and an event about anything else should.
    /// </summary>
    public FrameworkElement? CurrentPage =>
        PageHost.Children.Count > 0 ? PageHost.Children[^1] as FrameworkElement : null;

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
        BuildRail();
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
            UpdateRail(index, animate: false);
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

        UpdateRail(index, animate);
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

    /// <summary>
    /// Size the rail's travelling pip for the current page count.
    ///
    /// One page gets no rail at all: a position indicator for a place you cannot leave is
    /// chrome that only ever says "still here".
    /// </summary>
    /// <summary>
    /// A click anywhere along the rail goes to the page under the pointer.
    ///
    /// Paging has to be reachable by something other than knowing the surface takes a wheel,
    /// and this is what the dots used to be for. Hovering shows the rail as well, so it can be
    /// found by moving toward it rather than by already knowing it is there.
    /// </summary>
    private void WireRail()
    {
        Rail.MouseEnter += (_, _) => { if (_railVisible) ShowRailBriefly(); };
        Rail.MouseLeftButtonDown += (_, e) =>
        {
            if (Pager is null || _pages.Count < 2) return;

            var x = e.GetPosition(Rail).X;
            var index = (int)(x / (Rail.Width / _pages.Count));
            PageRequested?.Invoke(this, Math.Clamp(index, 0, _pages.Count - 1));
            e.Handled = true;
        };
    }

    private void BuildRail()
    {
        _railVisible = _pages.Count > 1;
        Rail.Visibility = _railVisible ? Visibility.Visible : Visibility.Collapsed;
        if (!_railVisible) return;

        RailPip.Width = Rail.Width / _pages.Count;
    }

    private bool _railVisible;

    /// <summary>How long the rail lingers after the last page change before fading out.</summary>
    private static readonly TimeSpan RailLinger = TimeSpan.FromMilliseconds(900);

    private DispatcherTimer? _railTimer;

    /// <summary>
    /// Move the pip to the current page, and show the rail while that is happening.
    ///
    /// The pip slides rather than jumping, on the same curve and duration as the page behind it,
    /// so the two read as one movement rather than a page turn with an indicator reacting to it.
    /// </summary>
    private void UpdateRail(int index, bool animate)
    {
        if (!_railVisible || Pager is null) return;

        var target = RailPip.Width * index;

        if (animate && IsAnimationAllowed)
        {
            RailPipShift.BeginAnimation(TranslateTransform.XProperty,
                new DoubleAnimation(target, TimeSpan.FromMilliseconds(SlideMs))
                {
                    EasingFunction = new CubicEase { EasingMode = EasingMode.EaseOut },
                });
        }
        else
        {
            RailPipShift.BeginAnimation(TranslateTransform.XProperty, null);
            RailPipShift.X = target;
        }

        ShowRailBriefly();
    }

    private void ShowRailBriefly()
    {
        Rail.BeginAnimation(OpacityProperty,
            new DoubleAnimation(1, TimeSpan.FromMilliseconds(140)));

        // Restarted rather than left to run: paging again while it is up should extend the
        // linger, not let the first change's timer take it away mid-swipe.
        _railTimer ??= new DispatcherTimer(DispatcherPriority.Background, Dispatcher);
        _railTimer.Stop();
        _railTimer.Interval = RailLinger;
        _railTimer.Tick -= OnRailLingerElapsed;
        _railTimer.Tick += OnRailLingerElapsed;
        _railTimer.Start();
    }

    private void OnRailLingerElapsed(object? sender, EventArgs e)
    {
        _railTimer?.Stop();
        Rail.BeginAnimation(OpacityProperty,
            new DoubleAnimation(0, TimeSpan.FromMilliseconds(320))
            {
                EasingFunction = new CubicEase { EasingMode = EasingMode.EaseIn },
            });
    }
}
