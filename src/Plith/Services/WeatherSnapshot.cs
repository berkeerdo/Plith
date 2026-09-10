namespace Plith.Services;

/// <summary>One reading of current conditions. Immutable and timestamped: the ambient row
/// shows it only while it is fresh, and freshness is decided against the fetch time rather
/// than against a "last updated" flag that a failed refresh would leave stale.</summary>
public readonly record struct WeatherSnapshot(double TemperatureC, int WeatherCode, DateTimeOffset FetchedAt);

/// <summary>
/// Turns Open-Meteo's WMO 4677 weather codes into something renderable, and decides whether
/// a snapshot is still worth showing.
///
/// Ranges rather than a case per code: WMO defines about thirty, most of which differ only in
/// intensity ("slight" vs "moderate" drizzle), and the row has space for one
/// short word. Collapsing them is a display decision, which is why it lives here and not in
/// the HTTP client.
/// </summary>
public static class WeatherCodeMap
{
    /// <summary>
    /// A short human-readable name for a WMO code.
    ///
    /// It used to return a Segoe MDL2 code point alongside the label, and that half is gone:
    /// the font ships different contents on different Windows builds, and seven of the ten
    /// glyphs originally chosen from it turned out not to exist at all and would have rendered
    /// as hollow "tofu" boxes on a user's machine. A font-existence test caught that and then
    /// had to keep guarding it; removing the glyphs removes the need for the guard. Weather is
    /// drawn as a sky now — see WeatherWidget — and a sky needs no icon.
    /// </summary>
    public static string Describe(int code) => code switch
    {
        0        => "Clear",
        1 or 2   => "Partly cloudy",
        3        => "Overcast",
        45 or 48 => "Fog",
        >= 51 and <= 57 => "Drizzle",
        >= 61 and <= 67 => "Rain",
        >= 71 and <= 77 => "Snow",
        >= 80 and <= 82 => "Showers",
        >= 85 and <= 86 => "Snow showers",
        >= 95 and <= 99 => "Thunderstorm",

        // Deliberately not an exception. Open-Meteo can add codes, and this runs on a
        // background refresh timer where a throw would silently stop every future refresh
        // for the sake of a decoration. Bounded above at 99 (WMO 4677's own ceiling) rather
        // than left open-ended: an unbounded ">= 95" swallowed every future or garbage code
        // above it into "Thunderstorm" instead of reaching this fallback at all.
        _ => "Unknown",
    };

    /// <summary>
    /// Whether a snapshot is still worth rendering. A stale reading is treated as absent
    /// rather than shown: a temperature from three hours ago is a wrong answer presented as
    /// a right one, and the column collapsing is honest.
    ///
    /// A future timestamp fails too. It is reachable through a clock change or a DST jump
    /// between the fetch and the render, and "negative age" would otherwise pass the
    /// comparison and pin a bad reading on screen until the next successful refresh.
    /// </summary>
    public static bool IsFresh(WeatherSnapshot snapshot, DateTimeOffset now, int maxAgeMinutes)
    {
        var age = now - snapshot.FetchedAt;
        return age >= TimeSpan.Zero && age <= TimeSpan.FromMinutes(maxAgeMinutes);
    }
}
