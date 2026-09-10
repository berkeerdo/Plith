namespace Plith.Services;

/// <summary>
/// Turns the two wheel messages that can mean "sideways" into one signed delta, where positive
/// always means "towards the next page".
///
/// Separate from the window hook, and pure, for one reason: the sign conventions are the part
/// most likely to be wrong, and they are exactly the part that cannot be checked by running the
/// app on this machine — the mouse here has no tilt wheel and no agent may drive input anyway.
/// A pure decoder can at least be pinned down by tests, so that when the behaviour is finally
/// measured on real hardware there is one place to change.
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
    ///   A plain <c>WM_MOUSEWHEEL</c> without the modifier is not a paging gesture and returns
    ///   null rather than 0, so the caller can tell "not for me" from "no movement".
    /// </summary>
    public static int? TryDecode(uint message, nint wParam) => message switch
    {
        WM_MOUSEHWHEEL => WheelDelta(wParam),
        WM_MOUSEWHEEL when HasShift(wParam) => -WheelDelta(wParam),
        _ => null,
    };

    /// <summary>The delta is the signed high word of wParam. Taking it as a short before
    /// widening is what makes a backwards scroll negative instead of ~65 000.</summary>
    private static int WheelDelta(nint wParam) => (short)((ulong)wParam >> 16 & 0xFFFF);

    private static bool HasShift(nint wParam) => ((ulong)wParam & 0xFFFF & MK_SHIFT) != 0;
}
