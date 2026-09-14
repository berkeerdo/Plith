using System.Windows;

namespace Plith.Views.Presentation;

/// <summary>
/// How the OSD rests, transitions and hit-tests. Deliberately three behaviours and no more.
///
/// OsdHost keeps everything else: band-window creation, the UIAccess z-band, the accent
/// resource mirror, positioning, the hide timer, hover keep-alive policy, suppression
/// wiring and position edit mode. Ambient Notch needs every one of those identically, which
/// is why it is a mode inside OsdHost rather than a second host — a second host would have
/// to re-earn each defect fixed during Phase 5 verification, including the fade-in
/// generation counter and the deliberately absent BeginAnimation(OpacityProperty, null).
/// </summary>
internal interface IOsdPresentation
{
    /// <summary>Distance Reposition() keeps from the working-area edge.</summary>
    double EdgeMarginDip { get; }

    /// <summary>True when the host shows nothing the user would read as "the OSD is up".
    ///
    /// The target is passed in rather than remembered because OsdOpacityPercent can change
    /// between shows: a remembered target is the one from the PREVIOUS transition, and
    /// comparing against it can send ShowOsd down the snap branch, which also skips
    /// Reposition(). See <see cref="IsFullyHidden"/> for the different question HideOsd asks.
    /// </summary>
    bool IsAtRest(double targetOpacity);

    /// <summary>True when nothing is on screen to take down. See PresentationPolicy.IsFullyHidden
    /// for why this is a different question from <see cref="IsAtRest"/>.</summary>
    bool IsFullyHidden { get; }

    /// <summary>Whether the window should accept mouse messages in its current state.</summary>
    bool WantsHitTesting { get; }

    /// <summary>Called once before a show transition begins, after Reposition().</summary>
    void PrepareShow();

    /// <summary>
    /// Animate to fully visible, invoking <paramref name="onCompleted"/> at the end.
    ///
    /// Must hand off from the current animated value rather than restarting from a base
    /// value — that hand-off is what makes interrupting a hide look continuous, and losing
    /// it is exactly the flicker defect fixed in Phase 5.
    /// </summary>
    void AnimateToVisible(double targetOpacity, Action onCompleted);

    /// <summary>Go fully visible with no animation. Used when a show arrives while already up.</summary>
    void SnapToVisible(double targetOpacity);

    /// <summary>Animate back to the resting state.</summary>
    void AnimateToRest(Action onCompleted);

    /// <summary>Called after OsdHost measures its content, so a mode whose resting state
    /// depends on content height can recompute it.</summary>
    void OnContentMeasured(Size contentSize);

    /// <summary>
    /// Take the window off the screen entirely, if this presentation can.
    ///
    /// Not the same as fading to zero, and the difference is measured in frames per second. A
    /// layered, topmost, UIAccess-band window is still a window at opacity 0: it stays in DWM's
    /// composition, and a game with anything in that band overlapping it loses independent flip
    /// and falls back to the composited path. Reported from 700 fps to 80.
    ///
    /// Classic can do this whenever it is at rest, because at rest it shows nothing. The notch
    /// cannot — its resting pill IS what it shows — so it declines, and the covering-window
    /// fallback is what gets a game back to Classic in the first place.
    /// </summary>
    void HideWindowIfPossible();
}
