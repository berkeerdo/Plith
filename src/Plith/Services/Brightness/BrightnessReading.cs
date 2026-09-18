namespace Plith.Services.Brightness;

/// <summary>
/// What a display reports about its own brightness.
/// </summary>
/// <param name="Min">The device's own floor. DDC/CI does not require it to be zero, and
/// assuming zero silently mis-scales every step on a monitor that reports otherwise.</param>
/// <param name="Current">Where it is now, in the device's own units.</param>
/// <param name="Max">The device's own ceiling.</param>
public readonly record struct BrightnessReading(int Min, int Current, int Max);
