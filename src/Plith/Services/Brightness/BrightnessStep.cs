namespace Plith.Services.Brightness;

/// <summary>
/// Where a brightness step lands. Pure arithmetic, so it is the part of the act half that is
/// properly testable without hardware.
/// </summary>
public static class BrightnessStep
{
    /// <summary>
    /// The value one step away from <paramref name="reading"/>, clamped to the device's own
    /// range.
    ///
    /// The step is a percentage of the device's span rather than a fixed number of units,
    /// because the span is whatever the device says it is.
    /// </summary>
    public static int Next(BrightnessReading reading, int stepPercent, bool up)
    {
        var span = reading.Max - reading.Min;
        if (span <= 0) return reading.Current;

        // At least one unit. A step that rounds to zero produces a key that appears broken
        // rather than a limit that has been reached.
        var delta = Math.Max(1, (int)Math.Round(span * (stepPercent / 100.0), MidpointRounding.AwayFromZero));
        var next = up ? reading.Current + delta : reading.Current - delta;

        return Math.Clamp(next, reading.Min, reading.Max);
    }
}
