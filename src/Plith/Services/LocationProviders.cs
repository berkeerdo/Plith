using System.Net.Http;
using System.Net.Http.Json;
using System.Text.Json.Serialization;
using Windows.Devices.Geolocation;

namespace Plith.Services;

/// <summary>
/// The Windows Location API. Tried first among the automatic sources because it is the
/// accurate one, and because asking for it is the only way the permission prompt ever
/// appears — a user who is never asked can never say yes.
///
/// Its behaviour inside a UIAccess process is unmeasured (see spec section 6), which is why
/// every path here is wrapped and falls through to a caller that has an IP fallback.
/// </summary>
public sealed class WindowsLocationProvider
{
    private readonly DiagnosticLog? _log;

    public WindowsLocationProvider(DiagnosticLog? log = null) => _log = log;

    public async Task<(LocationOutcome Outcome, GeoPoint? Point)> GetAsync(CancellationToken ct)
    {
        try
        {
            var access = await Geolocator.RequestAccessAsync();
            if (access != GeolocationAccessStatus.Allowed)
            {
                // Denied is permanent until the user changes it in Windows settings.
                // LocationResolver.ShouldRetryWindowsLocation reads this to stop re-asking.
                var outcome = access == GeolocationAccessStatus.Denied
                    ? LocationOutcome.Denied
                    : LocationOutcome.Unavailable;
                _log?.Info("Location", $"Windows Location access: {access}");
                return (outcome, null);
            }

            // City-level accuracy is all a weather column needs, and it is the tier Windows
            // grants with the least friction.
            var geo = new Geolocator { DesiredAccuracy = PositionAccuracy.Default };
            var pos = await geo.GetGeopositionAsync().AsTask(ct).ConfigureAwait(false);
            var p = pos?.Coordinate?.Point?.Position;
            if (p is null) return (LocationOutcome.Resolved, null);
            return (LocationOutcome.Resolved, new GeoPoint(p.Value.Latitude, p.Value.Longitude));
        }
        catch (Exception ex)
        {
            // Broad on purpose: this is a WinRT call from a UIAccess WPF process, the failure
            // surface is not enumerable from here, and every failure has the same handling —
            // fall through to IP. Logged at Info because it is an expected configuration, not
            // a fault.
            _log?.Info("Location", $"Windows Location unavailable: {ex.GetType().Name}");
            return (LocationOutcome.Unavailable, null);
        }
    }
}

/// <summary>
/// IP-based coordinates, the fallback for a denied or unavailable location service. Accurate
/// to roughly a city, which is the resolution the ambient row shows anyway.
/// </summary>
public sealed class IpLocationProvider : IDisposable
{
    private readonly HttpClient _http = new() { Timeout = TimeSpan.FromSeconds(10) };
    private readonly DiagnosticLog? _log;

    public IpLocationProvider(DiagnosticLog? log = null) => _log = log;

    public async Task<GeoPoint?> GetAsync(CancellationToken ct)
    {
        try
        {
            var r = await _http.GetFromJsonAsync<IpResponse>("https://ipapi.co/json/", ct).ConfigureAwait(false);
            if (r is null || r.Latitude == 0 && r.Longitude == 0) return null;
            return new GeoPoint(r.Latitude, r.Longitude);
        }
        catch (Exception ex) when (ex is HttpRequestException or TaskCanceledException or NotSupportedException or System.Text.Json.JsonException)
        {
            _log?.Info("Location", $"IP location failed: {ex.GetType().Name}");
            return null;
        }
    }

    public void Dispose() => _http.Dispose();

    private sealed class IpResponse
    {
        [JsonPropertyName("latitude")] public double Latitude { get; set; }
        [JsonPropertyName("longitude")] public double Longitude { get; set; }
    }
}
