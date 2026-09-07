using Plith.Services;

namespace Plith.Tests;

// Only the cache-invalidation seam is exercised here — everything else in WeatherService is
// the "gather" half (network calls, WinRT location, a DispatcherTimer), which spec §3 puts
// deliberately out of reach of the headless suite, the same split FullscreenVideoWatcher /
// FullscreenVideoDetector already follows. ShowWeather is kept false throughout so Start()'s
// fire-and-forget RefreshAsync returns before touching the network or WinRT location: the
// ShowWeather check is the first line inside the method and precedes every await.
public class WeatherServiceTests
{
    private static WeatherService Build(SettingsService settings) =>
        new(settings, new OpenMeteoClient(), new WindowsLocationProvider(), new IpLocationProvider());

    [Fact]
    public void ChangingWeatherLocation_ZeroesTheCachedCoordinates()
    {
        using var dir = new TempIniDir();
        var settings = new SettingsService(dir.IniPath);
        settings.Load();

        var seeded = settings.Current.Clone();
        seeded.ShowWeather = false;
        seeded.WeatherLocation = "Berlin";
        seeded.WeatherLatitude = 52.5;
        seeded.WeatherLongitude = 13.4;
        settings.Save(seeded);

        using var weather = Build(settings);
        weather.Start();

        // Simulates what SettingsWindow.ApplyFromUi actually produces: a Clone() of the
        // current model with WeatherLocation edited but WeatherLatitude/WeatherLongitude left
        // untouched, still carrying Berlin's coordinates.
        var edited = settings.Current.Clone();
        edited.WeatherLocation = "Paris";
        settings.Save(edited);

        Assert.Equal("Paris", settings.Current.WeatherLocation);
        Assert.Equal(0, settings.Current.WeatherLatitude);
        Assert.Equal(0, settings.Current.WeatherLongitude);
    }

    [Fact]
    public void SavingWithTheSameWeatherLocation_LeavesTheCachedCoordinatesAlone()
    {
        using var dir = new TempIniDir();
        var settings = new SettingsService(dir.IniPath);
        settings.Load();

        var seeded = settings.Current.Clone();
        seeded.ShowWeather = false;
        seeded.WeatherLocation = "Berlin";
        seeded.WeatherLatitude = 52.5;
        seeded.WeatherLongitude = 13.4;
        settings.Save(seeded);

        using var weather = Build(settings);
        weather.Start();

        // An unrelated settings save (e.g. dragging the opacity slider) must not disturb a
        // still-valid cache — only an actual WeatherLocation edit should.
        var unrelated = settings.Current.Clone();
        unrelated.OsdOpacityPercent = 80;
        settings.Save(unrelated);

        Assert.Equal(52.5, settings.Current.WeatherLatitude);
        Assert.Equal(13.4, settings.Current.WeatherLongitude);
    }

    [Fact]
    public void ChangingWeatherLocation_RaisesChangedABoundedNumberOfTimes()
    {
        using var dir = new TempIniDir();
        var settings = new SettingsService(dir.IniPath);
        settings.Load();

        var seeded = settings.Current.Clone();
        seeded.ShowWeather = false;
        seeded.WeatherLocation = "Berlin";
        seeded.WeatherLatitude = 52.5;
        seeded.WeatherLongitude = 13.4;
        settings.Save(seeded);

        using var weather = Build(settings);
        weather.Start();

        int changedCount = 0;
        settings.Changed += _ => changedCount++;

        // A single edit settles in exactly 2 Changed events: the user's own save, plus the
        // one corrective re-save OnSettingsChanged issues to zero the stale coordinates.
        // Termination is guaranteed by two INDEPENDENT guards, either of which alone would
        // stop the recursion: (1) the city-match check at the top of OnSettingsChanged, which
        // no-ops once _cachedLocationCity has caught up with the incoming WeatherLocation, and
        // (2) the "already zero" check guarding the nested Save, which no-ops once the
        // coordinates have been cleared regardless of what _cachedLocationCity holds. The
        // nested Save's own re-entrant Changed call is in fact caught by guard (2), not guard
        // (1) — reordering when _cachedLocationCity gets updated does not change this count,
        // because guard (2) catches the recursion either way. This test pins the event count
        // both guards together produce; it does not, by itself, distinguish which guard did
        // the catching.
        var edited = settings.Current.Clone();
        edited.WeatherLocation = "Paris";
        settings.Save(edited);

        Assert.Equal(2, changedCount);
    }

    [Fact]
    public void AfterDispose_ChangingWeatherLocation_NoLongerZeroesTheCache()
    {
        using var dir = new TempIniDir();
        var settings = new SettingsService(dir.IniPath);
        settings.Load();

        var seeded = settings.Current.Clone();
        seeded.ShowWeather = false;
        seeded.WeatherLocation = "Berlin";
        seeded.WeatherLatitude = 52.5;
        seeded.WeatherLongitude = 13.4;
        settings.Save(seeded);

        var weather = Build(settings);
        weather.Start();
        weather.Dispose();

        var edited = settings.Current.Clone();
        edited.WeatherLocation = "Paris";
        settings.Save(edited);

        Assert.Equal(52.5, settings.Current.WeatherLatitude);
        Assert.Equal(13.4, settings.Current.WeatherLongitude);
    }
}
