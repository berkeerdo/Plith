namespace Plith.Services;

/// <summary>
/// Turns the two wheel messages that can mean "sideways" into one signed delta, where positive
/// always means "towards the next page".
///
/// Separate from the window hook, and pure, because the sign conventions are the part most
/// likely to be wrong and a pure decoder can be pinned down by tests.
///
/// Its header used to add "and no agent may drive input anyway", which was wrong. A Debug build
/// runs at MEDIUM integrity — app.manifest sets uiAccess="false" and only Release swaps in the
/// signed manifest — so the notch takes scripted input like any other window. Measured on
/// 2026-09-19 by paging this decoder's own output with SendInput: one wheel notch, one page,
/// three times, logged as `Widget page committed: delta=120, index=N/4`. See
/// docs/SHELF-VERIFICATION.md §3.11. The WM_MOUSEHWHEEL path is still unmeasured: the mouse here
/// has no tilt wheel, and a touchpad's two-finger swipe delivers many small deltas rather than
/// one of 120, which is the case the pager's constants were actually chosen for.
/// </summary>
public static class WheelDecoder
{
    public const uint WM_MOUSEWHEEL = 0x020A;
    public const uint WM_MOUSEHWHEEL = 0x020E;

    private const int MK_SHIFT = 0x0004;

    /// <summary>
    /// Decode a wheel message into a paging delta, or null when the message is not a sideways
    /// gesture at all.
    ///
    /// Two routes reach the same place:
    ///
    /// - <c>WM_MOUSEHWHEEL</c> — a touchpad's two-finger horizontal swipe and a mouse's tilt
    ///   wheel produce this same message, which is the whole reason to build on it rather than
    ///   on a gesture API. Positive is right, which is also "next", so it passes through.
    ///
    /// - <c>WM_MOUSEWHEEL</c> with <c>MK_SHIFT</c> — the established Windows convention for
    ///   scrolling sideways on a mouse that has only a vertical wheel. Positive is forward,
    ///   away from the hand, which scrolls <em>left</em> by that convention, so it is negated.
    ///   A plain <c>WM_MOUSEWHEEL</c> pages as well. That was not true at first, on the
    ///   principle that a vertical wheel is not a sideways gesture — but the notch has nothing
    ///   of its own to scroll, so a wheel delivered to it can only have been meant for it, and
    ///   requiring a modifier to use the obvious control is a rule with no beneficiary.
    /// </summary>
    public static int? TryDecode(uint message, nint wParam) => message switch
    {
        WM_MOUSEHWHEEL => WheelDelta(wParam),

        // A plain vertical wheel pages too, and over this surface that costs nothing: the notch
        // has nothing of its own to scroll, so a wheel here can only have been meant for it.
        // Negated for the same reason the Shift path is - forward, away from the hand, is
        // "back", which is how every list a person has ever scrolled behaves.
        WM_MOUSEWHEEL => -WheelDelta(wParam),
        _ => null,
    };

    /// <summary>The delta is the signed high word of wParam. Taking it as a short before
    /// widening is what makes a backwards scroll negative instead of ~65 000.</summary>
    private static int WheelDelta(nint wParam) => (short)((ulong)wParam >> 16 & 0xFFFF);

    private static bool HasShift(nint wParam) => ((ulong)wParam & 0xFFFF & MK_SHIFT) != 0;
}
