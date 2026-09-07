using System.Net.Http;
using System.Net.Http.Json;
using System.Text.Json.Serialization;
using Windows.Devices.Geolocation;

namespace Plith.Services;

/// <summary>
/// The Windows Location API. Tried first among the automatic sources because it is the
/// accurate one.
///
/// Plith is unpackaged, so <see cref="Geolocator.RequestAccessAsync"/> never shows the
/// per-app consent dialog described in its own documentation — that dialog is a packaged-app
/// feature. For an unpackaged process, Windows Location access is governed entirely by the
/// system-wide "Allow desktop apps to access your location" toggle (and the master Location
/// switch), and every call here just reports their live state; there is no stored decision to
/// wait on or ask for.
///
/// Its behaviour inside a UIAccess process is unmeasured (see spec section 6), which is why
/// every path here is wrapped and falls through to a caller that has an IP fallback.
/// </summary>
public sealed class WindowsLocationProvider
{
    private readonly DiagnosticLog? _log;

    // See LogFailure/LogRecovery below — one remembered signature is enough because the
    // non-Allowed branch and the catch block are mutually exclusive per call.
    private string? _lastFailure;

    public WindowsLocationProvider(DiagnosticLog? log = null) => _log = log;

    public async Task<(LocationOutcome Outcome, GeoPoint? Point)> GetAsync(CancellationToken ct)
    {
        try
        {
            // RequestAccessAsync must run on the UI thread — Microsoft Learn: "You must call
            // this method on the UI thread, otherwise an exception will occur." This await is
            // deliberately left without ConfigureAwait(false), unlike every other await in
            // this file, so the method both starts and (on this call) resumes on the caller's
            // thread. Task 7's refresh timer must call GetAsync() directly from the
            // UI/dispatcher thread and must never wrap it in Task.Run: off the UI thread this
            // throws RPC_E_WRONG_THREAD, which the broad catch below swallows as Unavailable —
            // indistinguishable in the log from a real permission problem, so a threading bug
            // here would read as "location is off" forever.
            var access = await Geolocator.RequestAccessAsync();
            if (access != GeolocationAccessStatus.Allowed)
            {
                // Not a stored decision — see the class doc comment. Denied and Unavailable
                // are still reported separately for diagnostics (a toggle switch vs. a failed
                // or empty fix), but both are retried on schedule: WeatherService retries
                // Windows Location unconditionally on every refresh.
                var outcome = access == GeolocationAccessStatus.Denied
                    ? LocationOutcome.Denied
                    : LocationOutcome.Unavailable;
                LogFailure($"Windows Location access: {access}");
                return (outcome, null);
            }

            // City-level accuracy is all a weather column needs, and it is the tier Windows
            // grants with the least friction. Explicit age/timeout instead of the parameterless
            // overload, which times out after 60 seconds and can still be running when the next
            // refresh tick starts. 15 minutes matches the weather refresh cadence — a fix that
            // old is still worth using — and the 10-second timeout matches both HTTP clients in
            // this file and OpenMeteoClient.cs.
            var geo = new Geolocator { DesiredAccuracy = PositionAccuracy.Default };
            var pos = await geo.GetGeopositionAsync(TimeSpan.FromMinutes(15), TimeSpan.FromSeconds(10))
                .AsTask(ct).ConfigureAwait(false);
            var p = pos?.Coordinate?.Point?.Position;
            if (p is null) return (LocationOutcome.Resolved, null);
            LogRecovery("Windows Location recovered");
            return (LocationOutcome.Resolved, new GeoPoint(p.Value.Latitude, p.Value.Longitude));
        }
        catch (Exception ex)
        {
            // Broad on purpose: this is a WinRT call from a UIAccess WPF process, the failure
            // surface is not enumerable from here, and every failure has the same handling —
            // fall through to IP. Logged at Info because it is an expected configuration, not
            // a fault.
            LogFailure($"Windows Location unavailable: {ex.GetType().Name}");
            return (LocationOutcome.Unavailable, null);
        }
    }

    // Logs only when the failure signature changes, so a denial or outage held across many
    // refresh ticks produces one line, not one per tick — spec §3.3's "logged once per
    // transition, not once per attempt".
    private void LogFailure(string message)
    {
        if (string.Equals(_lastFailure, message, StringComparison.Ordinal)) return;
        _lastFailure = message;
        _log?.Info("Location", message);
    }

    // Logs recovery exactly once, the first successful fix after a logged failure, then clears
    // the remembered signature so a later failure logs again.
    private void LogRecovery(string message)
    {
        if (_lastFailure is null) return;
        _lastFailure = null;
        _log?.Info("Location", message);
    }
}

/// <summary>
/// IP-based coordinates, the fallback for a denied or unavailable location service. Accurate
/// to roughly a city, which is the resolution the ambient row shows anyway.
/// </summary>
public sealed class IpLocationProvider : IDisposable
{
    // PooledConnectionLifetime forces periodic DNS re-resolution — see the same comment on
    // OpenMeteoClient's HttpClient field. This provider is just as long-lived on the refresh
    // timer, and ipapi.co is just as capable of moving behind a new address over the app's
    // lifetime.
    private readonly HttpClient _http = new(new SocketsHttpHandler { PooledConnectionLifetime = TimeSpan.FromMinutes(5) })
        { Timeout = TimeSpan.FromSeconds(10) };
    private readonly DiagnosticLog? _log;
    private string? _lastFailure;

    public IpLocationProvider(DiagnosticLog? log = null) => _log = log;

    public async Task<GeoPoint?> GetAsync(CancellationToken ct)
    {
        try
        {
            var r = await _http.GetFromJsonAsync<IpResponse>("https://ipapi.co/json/", ct).ConfigureAwait(false);
            if (r is null || r.Latitude == 0 && r.Longitude == 0) return null;
            LogRecovery("IP location recovered");
            return new GeoPoint(r.Latitude, r.Longitude);
        }
        // OperationCanceledException, not TaskCanceledException — see the same comment on
        // OpenMeteoClient.GetCurrentAsync's catch filter; GetFromJsonAsync can throw the plain
        // base type while reading the body after headers arrive, and this filter must catch
        // that path too. ObjectDisposedException covers Dispose() racing an in-flight request
        // at shutdown.
        catch (Exception ex) when (ex is HttpRequestException or OperationCanceledException or ObjectDisposedException or NotSupportedException or System.Text.Json.JsonException)
        {
            LogFailure($"IP location failed: {ex.GetType().Name}");
            return null;
        }
    }

    public void Dispose() => _http.Dispose();

    // Logs only when the failure signature changes — spec §3.3's "logged once per transition,
    // not once per attempt".
    private void LogFailure(string message)
    {
        if (string.Equals(_lastFailure, message, StringComparison.Ordinal)) return;
        _lastFailure = message;
        _log?.Info("Location", message);
    }

    // Logs recovery exactly once, the first successful call after a logged failure.
    private void LogRecovery(string message)
    {
        if (_lastFailure is null) return;
        _lastFailure = null;
        _log?.Info("Location", message);
    }

    private sealed class IpResponse
    {
        [JsonPropertyName("latitude")] public double Latitude { get; set; }
        [JsonPropertyName("longitude")] public double Longitude { get; set; }
    }
}
