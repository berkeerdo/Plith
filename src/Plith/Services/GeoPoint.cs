namespace Plith.Services;

/// <summary>A coordinate, in degrees. Shared by every location source so the resolver can
/// compare them without knowing which one produced which.</summary>
public readonly record struct GeoPoint(double Latitude, double Longitude);

/// <summary>What happened when a location source was asked.
///
/// Denied and Unavailable are deliberately separate. Denied means the user said no, and is
/// permanent until they change it in Windows settings; Unavailable means the service was off
/// or had no fix yet, and can resolve on its own. Collapsing them into one failure case would
/// force a choice between re-prompting a user who declined and never recovering from a
/// transient outage.</summary>
public enum LocationOutcome { Resolved, Denied, Unavailable }
