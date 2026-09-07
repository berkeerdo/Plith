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
    ///
    /// Always true, for every outcome including Denied. Plith is unpackaged, so
    /// <c>Geolocator.RequestAccessAsync</c> shows no per-app consent dialog at all — that
    /// dialog, and the "answer once and it stops asking" behaviour Microsoft documents for it
    /// (https://learn.microsoft.com/en-us/uwp/api/windows.devices.geolocation.geolocator.requestaccessasync:
    /// "After the first time they grant or deny permission, this method no longer prompts for
    /// permission"), is a packaged-app feature that does not apply here. For Plith, every call
    /// just re-reads the live state of Windows' system-wide location toggle, so re-asking never
    /// pesters anyone — there is no prompt to repeat — and is required for a user who flips
    /// that toggle back on mid-session to recover without restarting Plith. See
    /// <c>LocationOutcome</c>'s doc comment for the full reasoning.</summary>
    public static bool ShouldRetryWindowsLocation(LocationOutcome outcome) => true;
}
