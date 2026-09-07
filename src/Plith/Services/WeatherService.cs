using System.Windows.Threading;

namespace Plith.Services;

/// <summary>
/// Owns the weather refresh loop: resolve a location, fetch conditions, cache the result.
/// The gather half. Every decision it makes is delegated — LocationResolver picks the
/// coordinate, WeatherCodeMap decides whether the result is still fresh.
///
/// Refreshes on a timer rather than on hover, deliberately. A hover must open the notch
/// immediately; making it wait on a network call would put a variable delay in front of the
/// one interaction this whole slice exists for.
/// </summary>
public sealed class WeatherService : IDisposable
{
    private const int RefreshMinutes = 15;

    private readonly SettingsService _settings;
    private readonly OpenMeteoClient _weather;
    private readonly WindowsLocationProvider _windowsLocation;
    private readonly IpLocationProvider _ipLocation;
    private readonly DiagnosticLog? _log;
    private readonly DispatcherTimer _timer;
    private readonly CancellationTokenSource _cts = new();

    private LocationOutcome _lastWindowsOutcome = LocationOutcome.Unavailable;
    private GeoPoint? _resolved;
    private bool _refreshing;

    public WeatherService(
        SettingsService settings,
        OpenMeteoClient weather,
        WindowsLocationProvider windowsLocation,
        IpLocationProvider ipLocation,
        DiagnosticLog? log = null)
    {
        _settings = settings;
        _weather = weather;
        _windowsLocation = windowsLocation;
        _ipLocation = ipLocation;
        _log = log;

        _timer = new DispatcherTimer(DispatcherPriority.Background, Dispatcher.CurrentDispatcher)
        {
            Interval = TimeSpan.FromMinutes(RefreshMinutes),
        };
        _timer.Tick += (_, _) => _ = RefreshAsync();
    }

    /// <summary>The most recent successful reading, or null. Freshness is the caller's
    /// question, not this class's — see WeatherCodeMap.IsFresh.</summary>
    public WeatherSnapshot? Current { get; private set; }

    /// <summary>Raised on the UI dispatcher after a successful refresh.</summary>
    public event Action? Updated;

    public void Start()
    {
        _timer.Start();
        _ = RefreshAsync();   // do not make the user wait 15 minutes for the first reading
    }

    private async Task RefreshAsync()
    {
        // A slow fetch must not stack up behind the timer. Refreshes are idempotent and
        // dropping one costs nothing — the next tick is 15 minutes away.
        if (_refreshing) return;
        if (!_settings.Current.ShowWeather) return;
        _refreshing = true;
        try
        {
            _resolved ??= await ResolveLocationAsync().ConfigureAwait(true);
            if (_resolved is not { } at) return;

            var snap = await _weather.GetCurrentAsync(at, _cts.Token).ConfigureAwait(true);
            if (snap is null) return;

            Current = snap;
            Updated?.Invoke();
        }
        finally
        {
            _refreshing = false;
        }
    }

    private async Task<GeoPoint?> ResolveLocationAsync()
    {
        var m = _settings.Current;

        // The manual override, already geocoded and cached in config.ini. Both coordinates
        // zero means "not cached" — a real (0, 0) is in the Atlantic and nobody's city.
        GeoPoint? manual = null;
        if (!string.IsNullOrWhiteSpace(m.WeatherLocation))
        {
            manual = m.WeatherLatitude != 0 || m.WeatherLongitude != 0
                ? new GeoPoint(m.WeatherLatitude, m.WeatherLongitude)
                : await GeocodeAndCacheAsync(m.WeatherLocation).ConfigureAwait(true);
        }

        GeoPoint? windowsPoint = null;
        // Always retry Windows Location, for every prior outcome including Denied: Plith is
        // unpackaged, so Geolocator.RequestAccessAsync shows no per-app consent dialog and
        // never "stops asking after the first answer" the way Microsoft's docs describe for
        // packaged apps. Every call here just re-reads the live state of Windows' system-wide
        // location toggle, so a user who flips that toggle back on mid-session must be able to
        // recover on the next refresh without restarting Plith — see GeoPoint.cs and
        // WindowsLocationProvider's doc comments for the full reasoning.
        if (manual is null)
        {
            (_lastWindowsOutcome, windowsPoint) = await _windowsLocation.GetAsync(_cts.Token).ConfigureAwait(true);
        }

        GeoPoint? ipPoint = null;
        if (manual is null && windowsPoint is null)
            ipPoint = await _ipLocation.GetAsync(_cts.Token).ConfigureAwait(true);

        var chosen = LocationResolver.Choose(manual, _lastWindowsOutcome, windowsPoint, ipPoint);
        _log?.Info("Weather", chosen is null
            ? "No location available; the weather column stays collapsed."
            : $"Location resolved (windowsOutcome={_lastWindowsOutcome}, manual={manual is not null}).");
        return chosen;
    }

    private async Task<GeoPoint?> GeocodeAndCacheAsync(string city)
    {
        var p = await _weather.GeocodeAsync(city, _cts.Token).ConfigureAwait(true);
        if (p is not { } point) return null;

        var m = _settings.Current.Clone();
        m.WeatherLatitude = point.Latitude;
        m.WeatherLongitude = point.Longitude;
        _settings.Save(m);
        return point;
    }

    /// <summary>Drop the cached location so the next refresh resolves again. Called when the
    /// user edits the manual location in Settings.</summary>
    public void InvalidateLocation()
    {
        _resolved = null;
        _lastWindowsOutcome = LocationOutcome.Unavailable;
    }

    public void Dispose()
    {
        _timer.Stop();
        _cts.Cancel();
        _cts.Dispose();
    }
}
