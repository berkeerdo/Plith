namespace Plith.Services;

/// <summary>
/// Decides which coordinate the weather is fetched for. Pure: every source's outcome is
/// handed in, so the whole priority order is testable without a location sensor, a network,
/// or a permission prompt — none of which the headless suite has.
///
/// The order was a product decision, not a technical one: try Windows Location first among
/// the automatic options, because it is the accurate one and asking for it is the only way
/// the permission prompt ever appears; fall back to IP for the users who decline.
/// </summary>
public static class LocationResolver
{
    public static GeoPoint? Choose(
        GeoPoint? cachedManual,
        LocationOutcome windowsOutcome,
        GeoPoint? windowsPoint,
        GeoPoint? ipPoint)
    {
        // The user typed a city. Nothing the system reports overrides that.
        if (cachedManual is { } manual) return manual;

        // Checks the VALUE, not just the outcome. The Geolocator can report allowed access
        // and still hand back nothing — it times out, or has no fix yet — and trusting the
        // enum alone would return null there and disable weather on a machine that said yes.
        if (windowsOutcome == LocationOutcome.Resolved && windowsPoint is { } w) return w;

        return ipPoint;
    }

    /// <summary>Whether the Windows Location API is worth asking again on the next refresh.
    /// A denial is permanent until the user changes it in Windows settings; re-asking on a
    /// 15-minute timer would pester someone who already answered.</summary>
    public static bool ShouldRetryWindowsLocation(LocationOutcome outcome)
        => outcome != LocationOutcome.Denied;
}
