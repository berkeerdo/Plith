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
    // PooledConnectionLifetime forces periodic DNS re-resolution. Without it a pinned
    // connection outlives a CDN/DNS change behind api.open-meteo.com and every fetch fails
    // silently until the process restarts — this client is long-lived on a refresh timer, not
    // a one-shot call, so it has to outlive that kind of change on its own.
    private readonly HttpClient _http = new(new SocketsHttpHandler { PooledConnectionLifetime = TimeSpan.FromMinutes(5) })
        { Timeout = TimeSpan.FromSeconds(10) };
    private readonly DiagnosticLog? _log;

    // One remembered failure signature per endpoint, so a fetch that fails on every tick of a
    // 15-minute timer logs once on the way down and once on the way back up, per spec §3.3,
    // instead of once per attempt.
    private string? _lastCurrentFailure;
    private string? _lastGeocodeFailure;

    public OpenMeteoClient(DiagnosticLog? log = null) => _log = log;

    public async Task<WeatherSnapshot?> GetCurrentAsync(GeoPoint at, CancellationToken ct)
    {
        // InvariantCulture on both coordinates. A Turkish install formats 41.0 as "41,0",
        // which the API rejects — and this project's default user is on tr-TR.
        var url = "https://api.open-meteo.com/v1/forecast"
                + $"?latitude={at.Latitude.ToString("0.####", CultureInfo.InvariantCulture)}"
                + $"&longitude={at.Longitude.ToString("0.####", CultureInfo.InvariantCulture)}"
                + "&current=temperature_2m,weather_code"
                // The short forecast rides along in the SAME request, so it costs no extra call
                // and cannot disagree with the current conditions about where it is.
                //
                // forecast_days=3 rather than 2: the first day Open-Meteo returns is TODAY, and
                // the page wants the two days AFTER today. timezone=auto so that "today" means
                // the day at the location rather than at UTC, which for this user is three hours
                // adrift and would name tomorrow as today every evening.
                + "&daily=weather_code,temperature_2m_max,temperature_2m_min"
                + "&forecast_days=3&timezone=auto";
        try
        {
            var r = await _http.GetFromJsonAsync<ForecastResponse>(url, ct).ConfigureAwait(false);
            if (r?.Current is not { } c) return null;
            LogRecovery(ref _lastCurrentFailure, "OpenMeteo", "Current-conditions fetch recovered");
            return new WeatherSnapshot(c.Temperature, c.WeatherCode, DateTimeOffset.UtcNow,
                                       ReadDays(r.Daily));
        }
        // OperationCanceledException, not TaskCanceledException: GetFromJsonAsync can cancel
        // after the response headers arrive, while it is reading/deserializing the body, and
        // that path throws the plain base type, not the Task-flavoured subclass. Catching only
        // the subclass let that case — and a Dispose() racing an in-flight request at shutdown,
        // which throws ObjectDisposedException — escape into the caller's refresh timer, which
        // this class's own doc comment says must never happen.
        catch (Exception ex) when (ex is HttpRequestException or OperationCanceledException or ObjectDisposedException or NotSupportedException or System.Text.Json.JsonException)
        {
            LogFailure(ref _lastCurrentFailure, "OpenMeteo", $"Current-conditions fetch failed: {ExceptionText.Describe(ex)}");
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
            if (hit is null) return null;
            LogRecovery(ref _lastGeocodeFailure, "OpenMeteo", "Geocode recovered");
            return new GeoPoint(hit.Latitude, hit.Longitude);
        }
        // See the comment on the same filter in GetCurrentAsync above.
        catch (Exception ex) when (ex is HttpRequestException or OperationCanceledException or ObjectDisposedException or NotSupportedException or System.Text.Json.JsonException)
        {
            LogFailure(ref _lastGeocodeFailure, "OpenMeteo", $"Geocode failed for '{cityName}': {ExceptionText.Describe(ex)}");
            return null;
        }
    }

    public void Dispose() => _http.Dispose();

    // Logs only when the failure signature changes, so an outage held across many refresh
    // ticks produces one line, not one per tick — spec §3.3's "logged once per transition, not
    // once per attempt".
    private void LogFailure(ref string? lastFailure, string source, string message)
    {
        if (string.Equals(lastFailure, message, StringComparison.Ordinal)) return;
        lastFailure = message;
        _log?.Info(source, message);
    }

    // Logs recovery exactly once, the first successful call after a logged failure, then
    // clears the remembered signature so a later failure logs again.
    private void LogRecovery(ref string? lastFailure, string source, string message)
    {
        if (lastFailure is null) return;
        lastFailure = null;
        _log?.Info(source, message);
    }

    /// <summary>The daily block, as parallel arrays, which is how Open-Meteo returns it. The
    /// zipping rule lives in <see cref="WeatherDays"/>, where a test can reach it.</summary>
    private static IReadOnlyList<WeatherDay>? ReadDays(DailyBlock? daily)
        => WeatherDays.From(daily?.Time, daily?.WeatherCode, daily?.Max, daily?.Min);

    private sealed class ForecastResponse
    {
        [JsonPropertyName("current")] public CurrentBlock? Current { get; set; }
        [JsonPropertyName("daily")] public DailyBlock? Daily { get; set; }
    }

    private sealed class DailyBlock
    {
        [JsonPropertyName("time")] public List<string>? Time { get; set; }
        [JsonPropertyName("weather_code")] public List<int>? WeatherCode { get; set; }
        [JsonPropertyName("temperature_2m_max")] public List<double>? Max { get; set; }
        [JsonPropertyName("temperature_2m_min")] public List<double>? Min { get; set; }
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
