namespace Plith.Services;

/// <summary>What a marquee should do with one piece of text in one viewport.</summary>
/// <param name="Scroll">False means leave it exactly where it is.</param>
/// <param name="ShiftDip">How far to translate, negative because the text moves left to reveal
/// its tail. Zero when <paramref name="Scroll"/> is false.</param>
/// <param name="Duration">One leg of the travel — out, or back. Zero when not scrolling.</param>
public readonly record struct MarqueePlan(bool Scroll, double ShiftDip, TimeSpan Duration);

/// <summary>
/// Whether a title scrolls, how far, and how long it takes.
///
/// Pure and separate from the control, because the rules are the part worth pinning down and the
/// control is the part the suite cannot construct at all. Four of them, none optional:
///
/// 1. <b>Conditional.</b> Measured overflow starts it; anything that fits sits still. A marquee
///    that always runs is a moving ellipsis.
/// 2. <b>Constant speed, not constant duration.</b> A fixed duration makes a long title fly and a
///    short one crawl — the tell of a marquee nobody measured.
/// 3. <b>A floor on the duration</b>, so a title overflowing by four pixels does not twitch.
/// 4. <b>Reduced motion wins</b> over all of it.
/// </summary>
public static class MarqueeDecision
{
    /// <summary>DIP per second. Slow enough to read at a glance, fast enough that a long title
    /// finishes inside the time an open notch is realistically looked at.</summary>
    public const double SpeedDipPerSecond = 28;

    /// <summary>Shortest one-way travel. Below this the movement reads as a twitch rather than a
    /// scroll, which is worse than not moving at all.</summary>
    public static readonly TimeSpan MinDuration = TimeSpan.FromSeconds(2.5);

    /// <summary>
    /// Overflow smaller than this is treated as fitting.
    ///
    /// Text measurement and layout rounding disagree by fractions of a pixel routinely, and
    /// without a floor here a title that visually fits perfectly would scroll by half a pixel
    /// forever — a slow shimmer with no cause the reader can see.
    /// </summary>
    public const double OverflowToleranceDip = 1.0;

    public static MarqueePlan Plan(double textWidthDip, double viewportWidthDip, bool reducedMotion)
    {
        if (reducedMotion) return default;

        // A viewport that has not been measured yet is not an argument for scrolling. A control
        // can be asked this during its first layout pass, when the width is still zero, and
        // "everything overflows an empty viewport" would start every title scrolling on load.
        if (viewportWidthDip <= 0 || textWidthDip <= 0) return default;

        var overflow = textWidthDip - viewportWidthDip;
        if (overflow <= OverflowToleranceDip) return default;

        var seconds = overflow / SpeedDipPerSecond;
        var duration = TimeSpan.FromSeconds(seconds);
        if (duration < MinDuration) duration = MinDuration;

        return new MarqueePlan(true, -overflow, duration);
    }
}
