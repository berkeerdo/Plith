using System.Windows;
using System.Windows.Media.Animation;
using Plith.Interop;
using Plith.Services;

namespace Plith.Views.Presentation;

/// <summary>
/// A thin strip parked at the top edge that slides down into the full card on an event or
/// hover, then retracts. The window itself never moves and is never hidden: it stays at the
/// descended size and the content translates inside it.
/// </summary>
internal sealed class AmbientNotchPresentation : IOsdPresentation
{
    private const int DescendMs = 220;
    private const int RetractMs = 260;

    private readonly BandWindow _window;
    private readonly OsdContent _content;
    private readonly Func<double> _stripHeight;
    private readonly DiagnosticLog? _log;

    private double _hiddenOffset;
    private bool _hasMeasured;
    private bool _isParked = true;

    // A retraction in flight counts as at rest, so an event arriving mid-retract routes
    // ShowOsd down the animate branch and hands off from the current offset instead of
    // snapping. The snap branch also skips Reposition(), so this is not merely cosmetic.
    private bool _isRetracting;


    public AmbientNotchPresentation(BandWindow window, OsdContent content, Func<double> stripHeight, DiagnosticLog? log)
    {
        _window = window;
        _content = content;
        _stripHeight = stripHeight;
        _log = log;
    }

    public double EdgeMarginDip => PresentationPolicy.EdgeMarginDip(PresentationMode.AmbientNotch);

    // IsAtRest and IsFullyHidden deliberately consult different flags here. IsAtRest also
    // treats a retraction-in-flight as "at rest" so ShowOsd hands off from the live offset
    // instead of snapping past it (see _isRetracting). IsFullyHidden must NOT do that: a
    // retraction in flight still has pixels on screen (the card is mid-slide), so there is
    // still something for HideOsd to consider taking down.
    public bool IsAtRest(double targetOpacity) =>
        PresentationPolicy.IsAtRest(PresentationMode.AmbientNotch, _window.Opacity, targetOpacity, _isParked || _isRetracting);

    public bool IsFullyHidden =>
        PresentationPolicy.IsFullyHidden(PresentationMode.AmbientNotch, _window.Opacity, _isParked);

    public bool WantsHitTesting => PresentationPolicy.WantsHitTesting(PresentationMode.AmbientNotch, _isParked);

    public void OnContentMeasured(Size contentSize)
    {
        _hiddenOffset = NotchGeometry.HiddenOffset(contentSize.Height, OsdContent.ContentInsetDip);
        _hasMeasured = true;

        // Re-apply whichever rest state is actually in force if the content grew or shrank
        // (the media card appearing or going away) while the notch was resting. Without this,
        // a parked strip would show a slice of the middle of the card instead of its top edge
        // — or, worse, a retracted notch would silently re-park: Retract() has no callers yet,
        // but Task 8 wires it to "a window covers the monitor", and a later Reposition() must
        // not put the strip and the card's bottom sliver back on screen over that window.
        //
        // Not a bare assignment: reaching either rest state through an animation leaves that
        // animation holding ContentOffsetProperty with FillBehavior.HoldEnd, and animated-value
        // precedence outranks a local write — so assigning here would change nothing at all.
        // Park()/Retract() clear the animation first.
        if (_isParked) Park();
    }

    public void PrepareShow()
    {
        // The window is permanently visible in this mode, so Show() is a first-activation
        // concern rather than a per-event one. BandWindow.Show is idempotent.
        _window.Show();
        _content.SetStrip(visible: true, heightDip: _stripHeight());
    }

    public void AnimateToVisible(double targetOpacity, Action onCompleted)
    {
        _isParked = false;
        _isRetracting = false;
        _window.BeginAnimation(UIElement.OpacityProperty, null);
        _window.Opacity = targetOpacity;

        // From-less, exactly as in ClassicPresentation: hand off from wherever a retraction
        // in flight had got to, rather than snapping back to the parked offset first.
        var descend = new DoubleAnimation(NotchGeometry.DescendedOffset, TimeSpan.FromMilliseconds(DescendMs))
        {
            EasingFunction = new CubicEase { EasingMode = EasingMode.EaseOut },
        };
        descend.Completed += (_, _) => onCompleted();
        _content.BeginAnimation(OsdContent.ContentOffsetProperty, descend);
    }

    public void SnapToVisible(double targetOpacity)
    {
        _isParked = false;
        _isRetracting = false;
        _window.BeginAnimation(UIElement.OpacityProperty, null);
        _window.Opacity = targetOpacity;
        _content.BeginAnimation(OsdContent.ContentOffsetProperty, null);
        _content.ContentOffset = NotchGeometry.DescendedOffset;
    }

    public void AnimateToRest(Action onCompleted)
    {
        _isRetracting = true;
        var retract = new DoubleAnimation(_hiddenOffset, TimeSpan.FromMilliseconds(RetractMs))
        {
            EasingFunction = new CubicEase { EasingMode = EasingMode.EaseIn },
        };
        retract.Completed += (_, _) => { _isParked = true; _isRetracting = false; onCompleted(); };
        _content.BeginAnimation(OsdContent.ContentOffsetProperty, retract);
    }

    /// <summary>Park immediately with no animation, strip still showing. Used when settling
    /// into notch mode, where an animated descent-then-retract would be a pointless flash.</summary>
    public void Park()
    {
        _isRetracting = false;
        _isParked = true;

        if (!_hasMeasured)
        {
            // Reposition() early-returns before OnContentMeasured runs when the target screen
            // can't be resolved or the first layout pass measures zero. There is no known-good
            // offset to park at, and by this point ApplyPresentationMode/PrepareShow has
            // already shown the window with ContentOffset at 0 — collapsing the strip alone
            // would still leave the full card sitting on screen at full opacity, permanently.
            // Hide the whole window instead; the measured path below calls Show() again once
            // a real measurement lands, so the notch recovers rather than staying hidden.
            _log?.Warn("AmbientNotchPresentation", "Park() called before any measurement; hiding the window instead of parking at an unmeasured offset.");
            _content.SetStrip(visible: false, heightDip: _stripHeight());
            _window.Hide();
            return;
        }

        _window.Show();
        _content.BeginAnimation(OsdContent.ContentOffsetProperty, null);
        // Parks at the HIDDEN offset, not the resting one, so the card contributes no pixels
        // and NotchStrip is the only thing on screen.
        //
        // RestingOffset leaves the card's own bottom edge sitting exactly at y = stripHeight,
        // which put two surfaces in the same band: the dedicated strip, and a sliver of the
        // card's bottom — carrying that Border's corner radius and drop shadow with it. On a
        // running build that reads as the corner of a card poking out from under the top of
        // the screen, not as a notch. A notch is a deliberate shape; a leaked card edge is an
        // artefact. Reported directly by the user on the first real session, which is what
        // settled the open question section 1 of docs/PHASE6-VERIFICATION.md had recorded.
        //
        // Park and Retract now differ only in whether the strip itself is shown, which is the
        // distinction their names actually claim.
        _content.ContentOffset = _hiddenOffset;
        _content.SetStrip(visible: true, heightDip: _stripHeight());
    }

}
