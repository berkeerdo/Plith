using System.Globalization;
using System.Linq;

namespace Plith.Services;

/// <summary>
/// One day of the forecast: which day it is, what it will mostly be, and the two ends of it.
///
/// A day rather than an hour, because the page has room for two columns and nobody reads an
/// hourly series at a glance. The high comes first in the pair for the same reason it does
/// everywhere else people read weather.
/// </summary>
public readonly record struct WeatherDay(DateOnly Date, int WeatherCode, double MaxC, double MinC);

/// <summary>One reading of current conditions. Immutable and timestamped: the ambient row
/// shows it only while it is fresh, and freshness is decided against the fetch time rather
/// than against a "last updated" flag that a failed refresh would leave stale.
///
/// <paramref name="Days"/> is the short forecast that comes back in the SAME call as the
/// current conditions, so it costs no extra request. Empty when the response carried none:
/// the page draws the columns it has and nothing where it has none, which is the same rule the
/// place line follows.</summary>
public readonly record struct WeatherSnapshot(
    double TemperatureC,
    int WeatherCode,
    DateTimeOffset FetchedAt,
    IReadOnlyList<WeatherDay>? Days = null);

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

/// <summary>
/// Turns Open-Meteo's parallel daily arrays into days.
///
/// Out of the HTTP client and in here for the reason <see cref="WeatherCodeMap"/> is: this is
/// the part with rules in it, and the client is the part a test cannot reach. The rule that
/// matters is the ZIP: four arrays indexed by one counter is how a day ends up carrying another
/// day's weather code, and a response with a short array is a response this has to survive.
/// </summary>
public static class WeatherDays
{
    /// <summary>
    /// The next <paramref name="count"/> days after <paramref name="today"/>, or null.
    ///
    /// TODAY IS DROPPED, and that is the point rather than a detail: Open-Meteo's first day is
    /// today, and today is already the big number on both pages that draw a forecast. A column
    /// repeating it would be the same fact twice with different rounding.
    ///
    /// Null rather than an empty list, so a caller has one thing to check and collapses its row
    /// on it. Here rather than in either page because two pages now ask the same question, and
    /// the clock page asking it its own way is how the two would come to disagree about what
    /// "tomorrow" means at midnight.
    /// </summary>
    public static IReadOnlyList<WeatherDay>? Next(
        IReadOnlyList<WeatherDay>? days, int count, DateOnly today)
    {
        var next = days?.Where(d => d.Date > today).Take(count).ToList();
        return next is { Count: > 0 } ? next : null;
    }

    /// <summary>
    /// Zip the arrays, stopping at the shortest.
    ///
    /// Null rather than an empty list when nothing usable came back, so callers have one thing to
    /// check. A date that will not parse drops its own day and keeps the rest: one malformed entry
    /// is not a reason to throw a forecast away.
    /// </summary>
    public static IReadOnlyList<WeatherDay>? From(
        IReadOnlyList<string>? dates,
        IReadOnlyList<int>? codes,
        IReadOnlyList<double>? highs,
        IReadOnlyList<double>? lows)
    {
        if (dates is null || codes is null || highs is null || lows is null) return null;

        var count = Math.Min(dates.Count, Math.Min(codes.Count, Math.Min(highs.Count, lows.Count)));
        if (count == 0) return null;

        var days = new List<WeatherDay>(count);
        for (var i = 0; i < count; i++)
        {
            // InvariantCulture: the API answers in ISO, and a tr-TR install parses "2026-09-21"
            // as a date only by luck. This project's default user is on tr-TR.
            if (!DateOnly.TryParse(dates[i], CultureInfo.InvariantCulture,
                                   DateTimeStyles.None, out var date))
            {
                continue;
            }

            days.Add(new WeatherDay(date, codes[i], highs[i], lows[i]));
        }

        return days.Count == 0 ? null : days;
    }
}
