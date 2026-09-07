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
    // Session cache for the IP fallback only — re-asking ipapi.co every 15 minutes while
    // Windows Location stays unavailable adds nothing and risks its free-tier rate limit.
    // Windows Location itself is never cached this way: it must be asked fresh every refresh
    // so a user flipping the system toggle back on mid-session recovers on the next tick, not
    // just the first one. A later Windows success naturally takes precedence over this cached
    // point, because LocationResolver.Choose only falls through to ipPoint when windowsPoint
    // is null.
    private GeoPoint? _cachedIpPoint;
    private bool _refreshing;

    // The city WeatherLatitude/WeatherLongitude were geocoded for. Seeded from the persisted
    // WeatherLocation in Start() — the pair loaded from config.ini is self-consistent as long
    // as nothing has edited WeatherLocation at runtime yet this session, which was always true
    // before Task 8 added the Settings text box — and kept in sync by GeocodeAndCacheAsync
    // after every fresh geocode. OnSettingsChanged compares the incoming WeatherLocation
    // against this field to tell an edited city apart from an unrelated settings save.
    private string? _cachedLocationCity;

    // Same once-per-transition logging discipline as OpenMeteoClient/WindowsLocationProvider/
    // IpLocationProvider: one line when a refresh starts failing, one when it recovers, not
    // one per tick.
    private string? _lastRefreshFailure;

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
        _cachedLocationCity = _settings.Current.WeatherLocation;
        _settings.Changed += OnSettingsChanged;
        _timer.Start();
        _ = RefreshAsync();   // do not make the user wait 15 minutes for the first reading
    }

    // WeatherLocation changing at runtime, via the Task 8 Settings text box, invalidates the
    // geocode cache — both halves of it. Session caches (InvalidateLocation) go first; the
    // persisted WeatherLatitude/WeatherLongitude pair is zeroed and re-saved second, because
    // ResolveLocationAsync treats "both zero" as "not cached" and would otherwise read the
    // stale pair straight back out of config.ini on the very next resolve. Fires on every
    // Save, not just a location edit, so an unrelated settings change (a slider drag) must be
    // a no-op here — the _cachedLocationCity comparison is what makes that so.
    private void OnSettingsChanged(SettingsModel m)
    {
        if (string.Equals(m.WeatherLocation, _cachedLocationCity, StringComparison.Ordinal)) return;
        _cachedLocationCity = m.WeatherLocation;
        InvalidateLocation();

        if (m.WeatherLatitude == 0 && m.WeatherLongitude == 0) return;
        var cleared = m.Clone();
        cleared.WeatherLatitude = 0;
        cleared.WeatherLongitude = 0;
        _settings.Save(cleared);
    }

    private async Task RefreshAsync()
    {
        // A slow fetch must not stack up behind the timer. Refreshes are idempotent and
        // dropping one costs nothing — the next tick is 15 minutes away.
        if (_refreshing) return;
        if (!_settings.Current.ShowWeather)
        {
            // Drop any prior reading rather than leaving it on screen. Nothing can flip this
            // setting at runtime yet, but Task 8 adds the toggle, and AmbientCard keeps feeding
            // Current into Tick regardless — a stale snapshot would otherwise stay visible for
            // up to WeatherMaxAgeMinutes after the user turned weather off.
            Current = null;
            return;
        }
        _refreshing = true;
        try
        {
            var at = await ResolveLocationAsync().ConfigureAwait(true);
            if (at is not { } point) return;

            var snap = await _weather.GetCurrentAsync(point, _cts.Token).ConfigureAwait(true);
            if (snap is null) return;

            Current = snap;
            LogRecovery("Weather refresh recovered");
            Updated?.Invoke();
        }
        catch (Exception ex)
        {
            // Nothing else observes a fire-and-forget task's exception: this method is always
            // invoked as `_ = RefreshAsync()`, from both Start() and the timer tick, and
            // src/Plith has no TaskScheduler.UnobservedTaskException or
            // DispatcherUnhandledException handler to catch it elsewhere. Left unguarded, a
            // failure here — most plausibly SettingsService.Save throwing IOException out of
            // GeocodeAndCacheAsync because config.ini is locked by an editor or a sync client,
            // but also anything a WeatherService.Updated subscriber like AmbientCard throws —
            // would be completely invisible: the `finally` below still resets _refreshing so
            // the loop survives, but silently, with no record that anything went wrong. That
            // contradicts spec §3.3's "logged once per transition, not once per attempt",
            // which is why this reuses the same LogFailure/LogRecovery dedup the location
            // providers use rather than swallowing the exception outright.
            LogFailure($"Refresh failed: {ex.GetType().Name}: {ex.Message}");
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
        // Always retry Windows Location, on every refresh and for every prior outcome
        // including Denied — never skipped by an in-memory cache the way the IP fallback is
        // below. It is a local call, not a network one, and it is documented not to re-prompt
        // after the first answer for an unpackaged app like Plith (see GeoPoint.cs and
        // WindowsLocationProvider's doc comments), which is exactly why asking again is free.
        // That is the whole point of the recovery case: a user who flips the system location
        // toggle back on mid-session must see it picked up on the next 15-minute tick, not
        // only on whichever refresh happened to run first.
        if (manual is null)
        {
            (_lastWindowsOutcome, windowsPoint) = await _windowsLocation.GetAsync(_cts.Token).ConfigureAwait(true);
        }

        GeoPoint? ipPoint = null;
        if (manual is null && windowsPoint is null)
        {
            // Reused for the rest of the process once it succeeds, unlike Windows Location
            // above: re-asking ipapi.co every 15 minutes while Windows stays unavailable buys
            // nothing and risks its free-tier rate limit. If Windows Location later succeeds,
            // windowsPoint above is non-null and LocationResolver.Choose never reaches this
            // cached value at all — a Windows recovery always wins over a stale IP fix.
            _cachedIpPoint ??= await _ipLocation.GetAsync(_cts.Token).ConfigureAwait(true);
            ipPoint = _cachedIpPoint;
        }

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

        // Race: the user can edit WeatherLocation to a different city while this HTTP call is
        // in flight. If they did, `city` is no longer what WeatherLocation says, and this
        // answer belongs to a city the user has already moved off. Persisting it would stamp
        // the new city's config.ini entry with the old city's coordinates — config.ini would
        // self-correct on the next OnSettingsChanged re-entry, but only after this method had
        // already returned the wrong GeoPoint to ResolveLocationAsync, showing the old city's
        // weather under the new city's label for up to a full refresh cycle. Discard instead:
        // losing this geocode is fine, the next tick re-resolves against the city the user
        // actually wants.
        if (!string.Equals(city, _settings.Current.WeatherLocation, StringComparison.Ordinal))
        {
            return null;
        }

        var m = _settings.Current.Clone();
        m.WeatherLatitude = point.Latitude;
        m.WeatherLongitude = point.Longitude;
        // Save() below raises Changed synchronously; set this first so OnSettingsChanged sees
        // a match and treats this save as the cache catching up, not another edit.
        _cachedLocationCity = city;
        _settings.Save(m);
        return point;
    }

    /// <summary>Drop the cached IP fallback so the next refresh asks again. Called when the
    /// user edits the manual location in Settings — Windows Location itself needs no
    /// invalidation, since it is never cached across refreshes in the first place.</summary>
    public void InvalidateLocation()
    {
        _cachedIpPoint = null;
        _lastWindowsOutcome = LocationOutcome.Unavailable;
    }

    public void Dispose()
    {
        _settings.Changed -= OnSettingsChanged;
        _timer.Stop();
        _cts.Cancel();
        _cts.Dispose();
    }

    // Logs only when the failure signature changes, so a fault held across many refresh ticks
    // produces one line, not one per tick — spec §3.3's "logged once per transition, not once
    // per attempt".
    private void LogFailure(string message)
    {
        if (string.Equals(_lastRefreshFailure, message, StringComparison.Ordinal)) return;
        _lastRefreshFailure = message;
        _log?.Info("Weather", message);
    }

    // Logs recovery exactly once, the first successful refresh after a logged failure, then
    // clears the remembered signature so a later failure logs again.
    private void LogRecovery(string message)
    {
        if (_lastRefreshFailure is null) return;
        _lastRefreshFailure = null;
        _log?.Info("Weather", message);
    }
}
