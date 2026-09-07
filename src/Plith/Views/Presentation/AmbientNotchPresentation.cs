using System.Windows;
using System.Windows.Media.Animation;
using Plith.Interop;
using Plith.Services;

namespace Plith.Views.Presentation;

/// <summary>
/// A notch: one surface pinned to the top edge of the screen that grows from a narrow resting
/// pill into the full panel on an event or hover, then shrinks back. The window itself never
/// moves and is never hidden — only the shape inside it changes size.
///
/// The distinction from the first design matters and is not stylistic. That one parked a
/// full-width strip over a card and translated the card down from behind it: two objects
/// passing each other, which reads as a drawer opening. This one has a single object that
/// changes size, which is what a notch is. See NotchGeometry for the geometry itself.
/// </summary>
internal sealed class AmbientNotchPresentation : IOsdPresentation
{
    private const int ExpandMs = 340;
    private const int CollapseMs = 260;

    private readonly BandWindow _window;
    private readonly OsdContent _content;
    private readonly Func<double> _collapsedHeight;
    private readonly DiagnosticLog? _log;

    private bool _hasMeasured;
    private bool _isParked = true;

    // A collapse in flight counts as at rest, so an event arriving mid-collapse routes
    // ShowOsd down the animate branch and hands off from the current expansion instead of
    // snapping. The snap branch also skips Reposition(), so this is not merely cosmetic.
    private bool _isCollapsing;

    public AmbientNotchPresentation(BandWindow window, OsdContent content, Func<double> collapsedHeight, DiagnosticLog? log)
    {
        _window = window;
        _content = content;
        _collapsedHeight = collapsedHeight;
        _log = log;
    }

    public double EdgeMarginDip => PresentationPolicy.EdgeMarginDip(PresentationMode.AmbientNotch);

    // IsAtRest and IsFullyHidden deliberately consult different flags here. IsAtRest also
    // treats a collapse-in-flight as "at rest" so ShowOsd hands off from the live expansion
    // instead of snapping past it (see _isCollapsing). IsFullyHidden must NOT do that: a
    // collapse in flight still has pixels on screen (the panel is mid-shrink), so there is
    // still something for HideOsd to consider taking down.
    public bool IsAtRest(double targetOpacity) =>
        PresentationPolicy.IsAtRest(PresentationMode.AmbientNotch, _window.Opacity, targetOpacity, _isParked || _isCollapsing);

    /// <summary>
    /// Whether the notch is closed, read from the shape itself rather than from a flag.
    ///
    /// _isParked is written in AnimateToRest's Completed callback, and WPF raises no Completed
    /// for a clock a competing animation replaced - the hazard behind five defects on this
    /// branch. A dropped callback leaves _isParked false forever, and everything derived from it
    /// is then wrong forever: a closed notch that believes it is open keeps the window
    /// hit-testable, so an invisible 440 DIP band across the top of the screen swallows every
    /// click meant for whatever is underneath. Reported on a running build exactly that way.
    ///
    /// Re-deriving IsClickThrough on a timer did not fix it, because the re-derivation read the
    /// same stale flag. NotchExpand is the animated value the shape is actually drawn from, so it
    /// cannot be stale: if the panel is closed this is zero, whatever any callback did or did not
    /// do. _isParked stays for transition bookkeeping, where a dropped callback is recoverable.
    /// </summary>
    private bool IsClosedNow => _content.NotchExpand <= 0.01;

    public bool IsFullyHidden =>
        PresentationPolicy.IsFullyHidden(PresentationMode.AmbientNotch, _window.Opacity, IsClosedNow);

    public bool WantsHitTesting => PresentationPolicy.WantsHitTesting(PresentationMode.AmbientNotch, IsClosedNow);

    public void OnContentMeasured(Size contentSize)
    {
        _content.SetNotchMetrics(_collapsedHeight(), contentSize);
        _hasMeasured = true;

        // No re-park needed after a content change, unlike the translate design this replaced.
        // There the resting position was derived from the measured height, so a media card
        // appearing while parked left a stale offset that exposed a slice of the card. Here the
        // resting shape is the collapsed pill, whose size does not depend on the measurement at
        // all — SetNotchMetrics has already re-applied the current expansion, and at rest that
        // recomputes to the same pill it was already showing.
    }

    public void PrepareShow()
    {
        // The window is permanently visible in this mode, so Show() is a first-activation
        // concern rather than a per-event one. BandWindow.Show is idempotent.
        _window.Show();
    }

    public void AnimateToVisible(double targetOpacity, Action onCompleted)
    {
        _isParked = false;
        _isCollapsing = false;
        _window.BeginAnimation(UIElement.OpacityProperty, null);
        _window.Opacity = targetOpacity;

        // From-less, exactly as in ClassicPresentation: hand off from wherever a collapse in
        // flight had got to, rather than snapping shut and reopening.
        //
        // Quintic rather than cubic, and ease-out only: the shape should leave the resting pill
        // fast and spend most of the duration settling. That decelerating tail is what reads as
        // springy. A genuine overshoot is unavailable here — the window is sized to the open
        // panel exactly, so a surface briefly larger than its final size would be clipped by the
        // window rather than seen.
        var expand = new DoubleAnimation(1.0, TimeSpan.FromMilliseconds(ExpandMs))
        {
            EasingFunction = new QuinticEase { EasingMode = EasingMode.EaseOut },
        };
        expand.Completed += (_, _) => onCompleted();
        _content.BeginAnimation(OsdContent.NotchExpandProperty, expand);
    }

    public void SnapToVisible(double targetOpacity)
    {
        _isParked = false;
        _isCollapsing = false;
        _window.BeginAnimation(UIElement.OpacityProperty, null);
        _window.Opacity = targetOpacity;
        _content.BeginAnimation(OsdContent.NotchExpandProperty, null);
        _content.NotchExpand = 1.0;
    }

    public void AnimateToRest(Action onCompleted)
    {
        _isCollapsing = true;
        var collapse = new DoubleAnimation(0.0, TimeSpan.FromMilliseconds(CollapseMs))
        {
            EasingFunction = new CubicEase { EasingMode = EasingMode.EaseIn },
        };
        collapse.Completed += (_, _) => { _isParked = true; _isCollapsing = false; onCompleted(); };
        _content.BeginAnimation(OsdContent.NotchExpandProperty, collapse);
    }

    /// <summary>Collapse immediately with no animation. Used when settling into notch mode,
    /// where an animated open-then-close would be a pointless flash.</summary>
    public void Park()
    {
        _isCollapsing = false;
        _isParked = true;

        if (!_hasMeasured)
        {
            // Reposition() early-returns before OnContentMeasured runs when the target screen
            // can't be resolved or the first layout pass measures zero. Unlike the translate
            // design this replaced, that is no longer a visual failure: the resting shape is
            // the collapsed pill, whose size comes from the user's setting and a constant, not
            // from the measurement — so parking unmeasured still puts exactly the right pill on
            // screen and nothing else. Worth a line anyway, because it means Reposition() bailed
            // and the panel this pill opens into has no size yet.
            _log?.Warn("AmbientNotchPresentation", "Park() called before any measurement; the resting pill is correct but the open panel has no measured size yet.");
        }

        _window.Show();

        // Not a bare assignment: reaching the collapsed state through an animation leaves that
        // animation holding NotchExpandProperty with FillBehavior.HoldEnd, and animated-value
        // precedence outranks a local write — so assigning without clearing the clock first
        // would change nothing at all.
        _content.BeginAnimation(OsdContent.NotchExpandProperty, null);
        _content.NotchExpand = 0.0;
    }
}
