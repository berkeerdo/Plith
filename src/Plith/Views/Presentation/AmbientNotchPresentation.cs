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

    private double _restingOffset;
    private bool _isParked = true;

    public AmbientNotchPresentation(BandWindow window, OsdContent content, Func<double> stripHeight)
    {
        _window = window;
        _content = content;
        _stripHeight = stripHeight;
    }

    public double EdgeMarginDip => PresentationPolicy.EdgeMarginDip(PresentationMode.AmbientNotch);

    public bool IsAtRest(double targetOpacity) =>
        PresentationPolicy.IsAtRest(PresentationMode.AmbientNotch, _window.Opacity, targetOpacity, _isParked);

    public bool IsFullyHidden =>
        PresentationPolicy.IsFullyHidden(PresentationMode.AmbientNotch, _window.Opacity, _isParked);

    public bool WantsHitTesting => PresentationPolicy.WantsHitTesting(PresentationMode.AmbientNotch, _isParked);

    /// <summary>True while only the strip is showing. Read by the hover poller and the
    /// retraction signal.</summary>
    public bool IsParked => _isParked;

    public void OnContentMeasured(Size contentSize)
    {
        _restingOffset = NotchGeometry.RestingOffset(
            contentSize.Height, _stripHeight(), OsdContent.ContentInsetDip);

        // Re-park at the new offset if the content grew or shrank (the media card appearing
        // or going away) while the notch was parked. Without this the strip would show a
        // slice of the middle of the card instead of its top edge.
        if (_isParked) _content.ContentOffset = _restingOffset;
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
        _window.BeginAnimation(UIElement.OpacityProperty, null);
        _window.Opacity = targetOpacity;
        _content.BeginAnimation(OsdContent.ContentOffsetProperty, null);
        _content.ContentOffset = NotchGeometry.DescendedOffset;
    }

    public void AnimateToRest(Action onCompleted)
    {
        var retract = new DoubleAnimation(_restingOffset, TimeSpan.FromMilliseconds(RetractMs))
        {
            EasingFunction = new CubicEase { EasingMode = EasingMode.EaseIn },
        };
        retract.Completed += (_, _) => { _isParked = true; onCompleted(); };
        _content.BeginAnimation(OsdContent.ContentOffsetProperty, retract);
    }

    /// <summary>Park immediately with no animation, strip still showing. Used when settling
    /// into notch mode, where an animated descent-then-retract would be a pointless flash.</summary>
    public void Park()
    {
        _content.BeginAnimation(OsdContent.ContentOffsetProperty, null);
        _content.ContentOffset = _restingOffset;
        _isParked = true;
        _content.SetStrip(visible: true, heightDip: _stripHeight());
    }

    /// <summary>Park immediately and hide the strip entirely. Used when a window covers the
    /// monitor — see the retraction signal in FullscreenVideoWatcher.</summary>
    public void Retract()
    {
        Park();
        _content.SetStrip(visible: false, heightDip: _stripHeight());
    }
}
