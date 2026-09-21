using Plith.Services;

namespace Plith.Tests;

public class WeatherDaysTests
{
    private static readonly string[] ThreeDates = ["2026-09-21", "2026-09-22", "2026-09-23"];

    [Fact]
    public void ZipsTheArraysIntoDays()
    {
        var days = WeatherDays.From(ThreeDates, [1, 61, 3], [21.0, 18.0, 23.0], [14.0, 12.0, 15.0]);

        Assert.NotNull(days);
        Assert.Equal(3, days!.Count);
        Assert.Equal(new DateOnly(2026, 9, 22), days[1].Date);
        Assert.Equal(61, days[1].WeatherCode);
        Assert.Equal(18.0, days[1].MaxC);
        Assert.Equal(12.0, days[1].MinC);
    }

    [Fact]
    public void StopsAtTheSHORTESTArray()
    {
        // The rule this class exists for. Indexing four arrays by one counter is how a day ends
        // up carrying another day's weather code, and a response with one short array is a
        // response that has to be survived rather than trusted.
        var days = WeatherDays.From(ThreeDates, [1, 61, 3], [21.0, 18.0], [14.0, 12.0, 15.0]);

        Assert.NotNull(days);
        Assert.Equal(2, days!.Count);
        Assert.Equal(new DateOnly(2026, 9, 22), days[1].Date);
        Assert.Equal(18.0, days[1].MaxC);
    }

    [Fact]
    public void AnUnparseableDateDropsItsOwnDayAndKeepsTheRest()
    {
        var days = WeatherDays.From(["2026-09-21", "not a date", "2026-09-23"],
                                    [1, 61, 3], [21.0, 18.0, 23.0], [14.0, 12.0, 15.0]);

        Assert.NotNull(days);
        Assert.Equal(2, days!.Count);
        Assert.Equal(new DateOnly(2026, 9, 21), days[0].Date);
        Assert.Equal(new DateOnly(2026, 9, 23), days[1].Date);
        // The second surviving day keeps ITS OWN values rather than the dropped day's.
        Assert.Equal(23.0, days[1].MaxC);
        Assert.Equal(3, days[1].WeatherCode);
    }

    [Fact]
    public void ParsesISODatesRegardlessOfTheCurrentCulture()
    {
        // The API answers in ISO and this project's default user is on tr-TR, where a
        // culture-sensitive parse of "2026-09-21" succeeds only by luck.
        var previous = System.Globalization.CultureInfo.CurrentCulture;
        try
        {
            System.Globalization.CultureInfo.CurrentCulture =
                new System.Globalization.CultureInfo("tr-TR");

            var days = WeatherDays.From(["2026-09-21"], [1], [21.0], [14.0]);

            Assert.NotNull(days);
            Assert.Equal(new DateOnly(2026, 9, 21), days![0].Date);
        }
        finally
        {
            System.Globalization.CultureInfo.CurrentCulture = previous;
        }
    }

    [Fact]
    public void AnyMissingArrayIsNothingAtAll()
    {
        Assert.Null(WeatherDays.From(null, [1], [21.0], [14.0]));
        Assert.Null(WeatherDays.From(ThreeDates, null, [21.0], [14.0]));
        Assert.Null(WeatherDays.From(ThreeDates, [1], null, [14.0]));
        Assert.Null(WeatherDays.From(ThreeDates, [1], [21.0], null));
    }

    [Fact]
    public void EmptyArraysAreNothingAtAll()
    {
        // Null rather than an empty list, so a caller has ONE thing to check. The page hides its
        // forecast columns on null and would otherwise have to hide them on Count == 0 as well.
        Assert.Null(WeatherDays.From([], [], [], []));
    }

    [Fact]
    public void EveryDateBeingUnparseableIsNothingAtAll()
    {
        Assert.Null(WeatherDays.From(["nope", "also nope"], [1, 2], [21.0, 22.0], [14.0, 15.0]));
    }
}
