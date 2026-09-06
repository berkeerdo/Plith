using System.Windows;
using System.Windows.Media.Animation;
using Plith.Interop;
using Plith.Services;

namespace Plith.Views.Presentation;

/// <summary>
/// The OSD as it behaved through 0.1.5: invisible at rest, opacity fade in and out at a
/// fixed anchor. Nothing here is new — the animations are moved from OsdHost unchanged so
/// that selecting Classic is byte-identical to the previous release.
/// </summary>
internal sealed class ClassicPresentation : IOsdPresentation
{
    private const int FadeInMs = 140;
    private const int FadeOutMs = 220;

    private readonly BandWindow _window;

    public ClassicPresentation(BandWindow window) => _window = window;

    public double EdgeMarginDip => PresentationPolicy.EdgeMarginDip(PresentationMode.ClassicOsd);

    public bool IsAtRest =>
        PresentationPolicy.IsAtRest(PresentationMode.ClassicOsd, _window.Opacity, TargetOpacity, isParked: false);

    public bool WantsHitTesting => PresentationPolicy.WantsHitTesting(PresentationMode.ClassicOsd, isParked: false);

    // Recorded on each transition so the policy above and the animations below agree on what
    // "fully visible" means for the current settings. OsdOpacityPercent can be as low as 50,
    // so "at rest" has to mean "below the target", not "below 1.0".
    public double TargetOpacity { get; private set; } = 1.0;

    public void PrepareShow() => _window.Show();

    public void AnimateToVisible(double targetOpacity, Action onCompleted)
    {
        TargetOpacity = targetOpacity;

        // Note the absence of BeginAnimation(OpacityProperty, null) here. Clearing an
        // animation reverts the property to its base value, which is 0 while the window is
        // hidden, so a clear-then-restart snapped the OSD back to fully invisible on every
        // repeat. A From-less DoubleAnimation hands off from the current animated value,
        // which is what makes interrupting a fade-out look continuous.
        var fadeIn = new DoubleAnimation(targetOpacity, TimeSpan.FromMilliseconds(FadeInMs))
        {
            EasingFunction = new CubicEase { EasingMode = EasingMode.EaseOut },
        };
        fadeIn.Completed += (_, _) => onCompleted();
        _window.BeginAnimation(UIElement.OpacityProperty, fadeIn);
    }

    public void SnapToVisible(double targetOpacity)
    {
        TargetOpacity = targetOpacity;
        _window.BeginAnimation(UIElement.OpacityProperty, null);
        _window.Opacity = targetOpacity;
    }

    public void AnimateToRest(Action onCompleted)
    {
        var fadeOut = new DoubleAnimation(0.0, TimeSpan.FromMilliseconds(FadeOutMs))
        {
            EasingFunction = new CubicEase { EasingMode = EasingMode.EaseIn },
        };
        fadeOut.Completed += (_, _) => onCompleted();
        _window.BeginAnimation(UIElement.OpacityProperty, fadeOut);
    }

    public void OnContentMeasured(Size contentSize) { /* Classic's resting state is content-independent. */ }
}
