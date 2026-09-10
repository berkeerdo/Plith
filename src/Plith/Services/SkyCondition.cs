namespace Plith.Services;

/// <summary>
/// What the weather page's sky looks like. Not the same axis as
/// <see cref="WeatherCodeMap.Describe"/>: that names the weather for a person to read, this
/// decides what is drawn, and several distinct conditions share one sky.
/// </summary>
public enum SkyKind
{
    /// <summary>Blue, with the sun blooming off-centre.</summary>
    Clear,

    /// <summary>Flatter and greyer, no sun. Covers overcast and fog.</summary>
    Overcast,

    /// <summary>Dark, with falling streaks. Covers drizzle, rain, showers and thunder.</summary>
    Rain,

    /// <summary>Pale and cold, with falling flakes.</summary>
    Snow,

    /// <summary>Deep blue, with a moon. Wins over Clear after dark; the wet skies do not care
    /// about the hour, because rain at night still reads as rain.</summary>
    Night,
}

/// <summary>
/// Weather code plus time of day to a sky.
///
/// Pure and tested, unlike everything it draws. The mapping is the part that can be wrong in a
/// way nobody notices — a code group silently falling into the wrong arm shows a blue sky during
/// a thunderstorm, and nothing about that fails a build.
/// </summary>
public static class SkyCondition
{
    /// <summary>
    /// The sky for a weather code at a local hour.
    ///
    /// Night is decided by the hour rather than by real sunrise and sunset. Open-Meteo will hand
    /// those over for the asking, and it should later; a fixed window is wrong by up to an hour
    /// or two near the solstices at this latitude. It is chosen deliberately for now because the
    /// alternative is another field to fetch, cache, and get stale — and being an hour out on
    /// the colour of a gradient is a smaller error than showing a stale one.
    /// </summary>
    public static SkyKind From(int weatherCode, int localHour)
    {
        var wet = Wet(weatherCode);

        // Rain and snow ignore the hour. A wet sky at night is still a wet sky, and a night
        // gradient with rain streaks over it reads as neither.
        if (wet is not null) return wet.Value;

        if (IsNight(localHour)) return SkyKind.Night;

        return weatherCode switch
        {
            0 or 1 => SkyKind.Clear,
            _ => SkyKind.Overcast,
        };
    }

    /// <summary>Whether the sky is falling with something, and what. Null means it is not.</summary>
    private static SkyKind? Wet(int weatherCode) => weatherCode switch
    {
        >= 51 and <= 67 => SkyKind.Rain,
        >= 71 and <= 77 => SkyKind.Snow,
        >= 80 and <= 82 => SkyKind.Rain,
        >= 85 and <= 86 => SkyKind.Snow,

        // Bounded above at 99, WMO 4677's own ceiling. An unbounded ">= 95" is the exact defect
        // WeatherCodeMap already carried once: it swallowed every future or garbage code into
        // Thunderstorm and made the fallback unreachable.
        >= 95 and <= 99 => SkyKind.Rain,
        _ => null,
    };

    /// <summary>Hours at or after <see cref="NightStartsHour"/>, or before
    /// <see cref="DayStartsHour"/>, are night.</summary>
    public const int NightStartsHour = 20;
    public const int DayStartsHour = 6;

    public static bool IsNight(int localHour) =>
        localHour >= NightStartsHour || localHour < DayStartsHour;
}
