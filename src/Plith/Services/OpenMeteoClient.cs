using System.Globalization;
using System.Net.Http;
using System.Net.Http.Json;
using System.Text.Json.Serialization;

namespace Plith.Services;

/// <summary>
/// Open-Meteo current-conditions and geocoding calls.
///
/// Open-Meteo rather than a keyed provider because a key would have to either ship in the
/// binary or be typed by the user, and neither is acceptable for a column that is decoration.
///
/// Every method returns null on any failure and throws nothing. These run on a background
/// refresh timer where an escaping exception would stop all future refreshes, and the caller's
/// contract is already "no snapshot means collapse the column".
/// </summary>
public sealed class OpenMeteoClient : IDisposable
{
    private readonly HttpClient _http = new() { Timeout = TimeSpan.FromSeconds(10) };
    private readonly DiagnosticLog? _log;

    public OpenMeteoClient(DiagnosticLog? log = null) => _log = log;

    public async Task<WeatherSnapshot?> GetCurrentAsync(GeoPoint at, CancellationToken ct)
    {
        // InvariantCulture on both coordinates. A Turkish install formats 41.0 as "41,0",
        // which the API rejects — and this project's default user is on tr-TR.
        var url = "https://api.open-meteo.com/v1/forecast"
                + $"?latitude={at.Latitude.ToString("0.####", CultureInfo.InvariantCulture)}"
                + $"&longitude={at.Longitude.ToString("0.####", CultureInfo.InvariantCulture)}"
                + "&current=temperature_2m,weather_code";
        try
        {
            var r = await _http.GetFromJsonAsync<ForecastResponse>(url, ct).ConfigureAwait(false);
            if (r?.Current is not { } c) return null;
            return new WeatherSnapshot(c.Temperature, c.WeatherCode, DateTimeOffset.UtcNow);
        }
        catch (Exception ex) when (ex is HttpRequestException or TaskCanceledException or NotSupportedException or System.Text.Json.JsonException)
        {
            _log?.Info("OpenMeteo", $"Current-conditions fetch failed: {ex.GetType().Name}");
            return null;
        }
    }

    public async Task<GeoPoint?> GeocodeAsync(string cityName, CancellationToken ct)
    {
        if (string.IsNullOrWhiteSpace(cityName)) return null;
        var url = "https://geocoding-api.open-meteo.com/v1/search"
                + $"?name={Uri.EscapeDataString(cityName)}&count=1";
        try
        {
            var r = await _http.GetFromJsonAsync<GeocodeResponse>(url, ct).ConfigureAwait(false);
            var hit = r?.Results?.FirstOrDefault();
            return hit is null ? null : new GeoPoint(hit.Latitude, hit.Longitude);
        }
        catch (Exception ex) when (ex is HttpRequestException or TaskCanceledException or NotSupportedException or System.Text.Json.JsonException)
        {
            _log?.Info("OpenMeteo", $"Geocode failed for '{cityName}': {ex.GetType().Name}");
            return null;
        }
    }

    public void Dispose() => _http.Dispose();

    private sealed class ForecastResponse
    {
        [JsonPropertyName("current")] public CurrentBlock? Current { get; set; }
    }

    private sealed class CurrentBlock
    {
        [JsonPropertyName("temperature_2m")] public double Temperature { get; set; }
        [JsonPropertyName("weather_code")] public int WeatherCode { get; set; }
    }

    private sealed class GeocodeResponse
    {
        [JsonPropertyName("results")] public List<GeocodeHit>? Results { get; set; }
    }

    private sealed class GeocodeHit
    {
        [JsonPropertyName("latitude")] public double Latitude { get; set; }
        [JsonPropertyName("longitude")] public double Longitude { get; set; }
    }
}
