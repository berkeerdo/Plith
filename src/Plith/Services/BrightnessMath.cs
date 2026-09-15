namespace Plith.Services;

/// <summary>
/// Converting between a display's own brightness units and the 0..100 everything above
/// <see cref="BrightnessClient"/> speaks.
///
/// Pulled out of the client rather than left as two private methods, for one reason: the cases
/// that matter cannot be reproduced on the machine this was written on. The monitor here reports
/// a 0..100 range, so the identity case is the only one a running build can exercise — while
/// panels reporting 0..10, and panels reporting a non-zero minimum because they cannot go fully
/// dark, are exactly where an off-by-one lives. A pure function is the only honest way to cover
/// a display nobody here owns.
/// </summary>
public static class BrightnessMath
{
    /// <summary>
    /// A raw reading in the display's own units to 0..100.
    ///
    /// Out-of-range readings are clamped rather than trusted. A display that reports a current
    /// value outside the range it just described is misbehaving, and the useful response is the
    /// nearest sane number rather than a percentage above 100 that a slider would then refuse.
    /// </summary>
    public static int ToPercent(uint raw, uint min, uint max)
    {
        // A range of zero width is a display saying its brightness cannot move. Dividing by it
        // would be the crash; answering with the bottom of the range is what it actually means.
        if (max <= min) return 0;

        var clamped = Math.Clamp(raw, min, max);
        return (int)Math.Round((clamped - min) * 100.0 / (max - min));
    }

    /// <summary>
    /// A percentage to the display's own units.
    ///
    /// Not an exact inverse of <see cref="ToPercent"/>, and cannot be on a coarse range: a panel
    /// reporting 0..10 has eleven reachable values, so percentages round to the nearest of them
    /// and several map to the same one. Round-tripping a percentage through both therefore lands
    /// on the nearest REACHABLE percentage rather than the one it started from — which is the
    /// display's granularity showing through, not a defect to be papered over.
    /// </summary>
    public static uint ToRaw(int percent, uint min, uint max)
    {
        if (max <= min) return min;

        var clamped = Math.Clamp(percent, 0, 100);
        return min + (uint)Math.Round(clamped / 100.0 * (max - min));
    }
}
