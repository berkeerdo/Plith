using Plith.Services;

namespace Plith.Tests;

public class LocationResolverTests
{
    private static readonly GeoPoint Manual = new(41.0, 29.0);
    private static readonly GeoPoint Windows = new(52.0, 13.0);
    private static readonly GeoPoint Ip = new(48.0, 2.0);

    [Fact]
    public void ManualOverrideBeatsEverything()
    {
        // The user typed a city. Nothing the system reports should override that, including a
        // successful and more accurate Windows Location fix.
        Assert.Equal(Manual, LocationResolver.Choose(Manual, LocationOutcome.Resolved, Windows, Ip));
    }

    [Fact]
    public void WindowsLocationBeatsIpWhenItResolves()
    {
        Assert.Equal(Windows, LocationResolver.Choose(null, LocationOutcome.Resolved, Windows, Ip));
    }

    [Fact]
    public void FallsBackToIpWhenWindowsLocationIsDenied()
    {
        Assert.Equal(Ip, LocationResolver.Choose(null, LocationOutcome.Denied, null, Ip));
    }

    [Fact]
    public void FallsBackToIpWhenWindowsLocationIsUnavailable()
    {
        Assert.Equal(Ip, LocationResolver.Choose(null, LocationOutcome.Unavailable, null, Ip));
    }

    [Fact]
    public void FallsBackToIpWhenWindowsReportsResolvedButHandsBackNothing()
    {
        // Reachable: the Geolocator can report allowed access and then time out or throw
        // while producing the position. Trusting the outcome enum over the actual value
        // would return null here and disable weather on a machine that granted permission.
        Assert.Equal(Ip, LocationResolver.Choose(null, LocationOutcome.Resolved, null, Ip));
    }

    [Fact]
    public void ReturnsNullWhenNothingIsAvailable()
    {
        Assert.Null(LocationResolver.Choose(null, LocationOutcome.Unavailable, null, null));
    }

    [Fact]
    public void RetriesWindowsLocationAfterADenialToo()
    {
        // Denied is not a stored decision for Plith: it is unpackaged, so
        // Geolocator.RequestAccessAsync shows no per-app consent dialog at all, and this
        // outcome just reflects the live state of Windows' system-wide location toggle. A user
        // who flips that toggle back on mid-session must be able to recover without restarting
        // Plith, so retrying costs nothing and is required.
        Assert.True(LocationResolver.ShouldRetryWindowsLocation(LocationOutcome.Denied));
    }

    [Fact]
    public void RetriesWindowsLocationAfterATransientFailure()
    {
        // Unavailable covers "the service was off" and "the sensor had no fix yet", both of
        // which can become available later without the user doing anything.
        Assert.True(LocationResolver.ShouldRetryWindowsLocation(LocationOutcome.Unavailable));
    }
}
