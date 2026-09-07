namespace Plith.Services;

/// <summary>A coordinate, in degrees. Shared by every location source so the resolver can
/// compare them without knowing which one produced which.</summary>
public readonly record struct GeoPoint(double Latitude, double Longitude);

/// <summary>What happened when a location source was asked.
///
/// Denied and Unavailable are kept separate for diagnostics — Denied means Windows' "Allow
/// desktop apps to access your location" toggle (or the master Location switch) was off at the
/// moment of the call; Unavailable means the Geolocator call itself failed or returned no fix.
/// Neither is a stored decision to honor going forward: Plith is an unpackaged process, and
/// <c>Geolocator.RequestAccessAsync</c> shows a per-app consent prompt only for packaged apps
/// (see <c>WindowsLocationProvider</c>'s doc comment) — for Plith it just reports the live
/// state of those system settings each time it is called, with no memory of a previous answer.
/// That is why <c>WeatherService</c> retries Windows Location unconditionally on every refresh,
/// after both outcomes: a user who flips the toggle back on mid-session must recover without
/// restarting Plith.
/// </summary>
public enum LocationOutcome { Resolved, Denied, Unavailable }
