namespace Plith.Views.Presentation;

/// <summary>
/// Decides whether a drag is arriving at the notch, from three booleans a poll can read.
///
/// Separated from <see cref="NotchHoverPoller"/> for the reason every decision in this namespace
/// is: the poller needs a Dispatcher and a live cursor, so anything inside it is unreachable from
/// the test suite. The gathering stays there, the rule lives here.
///
/// The rule itself is one sentence: a drag is arriving when the left button is down, it went down
/// somewhere other than the OSD, and the cursor has since reached the notch's approach band. The
/// middle clause is what separates a file being carried toward the notch from a press on the
/// notch itself — without it, every click on the notch would read as an incoming drop and hand
/// the screen to the catcher.
///
/// Entering and staying are two different rectangles, and that is not a refinement. Measured on a
/// live drag: the notch stood aside, the catcher appeared, DragEnter arrived — and 700 ms later
/// Plith took the screen back while the file was still in the air. The band that starts the
/// handoff is 190x28 DIP, the panel it opens is 356x116, and moving down into that panel to aim
/// the drop leaves the band. One threshold for both questions cannot answer either well.
///
/// This cannot tell a file drag from a window being dragged to the top of the screen, and nothing
/// readable from the cursor can: both are a held button moving to the same place. The catcher
/// settles it instead — a real file drag raises DragEnter there within a few frames, and the
/// catcher withdraws on its own when none arrives.
/// </summary>
internal sealed class DragApproachDetector
{
    private bool _buttonWasDown;
    private bool _pressStartedOutside;

    /// <summary>True while a drag that began elsewhere is over the approach band.</summary>
    public bool IsApproaching { get; private set; }

    /// <summary>Where the held button went down, for diagnostics. A drag that reaches the band
    /// and still does not register is either this or nothing, and from the outside the two are
    /// the same silence.</summary>
    public bool PressStartedOutside => _pressStartedOutside;

    /// <summary>
    /// Returns true when <see cref="IsApproaching"/> changed on this update, so the caller raises
    /// a transition rather than a state.
    /// </summary>
    /// <param name="cursorInApproachBand">The narrow band that STARTS a handoff. Narrow on
    /// purpose: it is at the very top of the screen and must not claim every gesture near the
    /// edge.</param>
    /// <param name="cursorInHoldBand">The rectangle the catcher occupies, which KEEPS one going.
    /// Wider, because by then the person is aiming at a panel rather than at an edge.</param>
    public bool Update(bool buttonDown, bool cursorOverOsd, bool cursorInApproachBand, bool cursorInHoldBand)
    {
        // Sampled on the rising edge only. Re-evaluating it every tick would flip the origin to
        // "inside" the moment the cursor reached the notch, which is precisely when the answer
        // has to still describe where the press began.
        if (buttonDown && !_buttonWasDown) _pressStartedOutside = !cursorOverOsd;
        _buttonWasDown = buttonDown;

        var stillThere = IsApproaching ? cursorInHoldBand : cursorInApproachBand;
        var approaching = buttonDown && _pressStartedOutside && stillThere;
        if (approaching == IsApproaching) return false;

        IsApproaching = approaching;
        return true;
    }

    /// <summary>
    /// Forget everything, for a poller that is stopping or starting.
    ///
    /// Without this, a poller stopped mid-drag and restarted later would still believe a button
    /// it never saw go down is held, and the first cursor to cross the band would hand the screen
    /// to the catcher with no drag in flight at all.
    /// </summary>
    public void Reset()
    {
        _buttonWasDown = false;
        _pressStartedOutside = false;
        IsApproaching = false;
    }
}
