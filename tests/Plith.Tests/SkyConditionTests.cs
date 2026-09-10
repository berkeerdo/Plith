using Plith.Services;

namespace Plith.Tests;

public class SkyConditionTests
{
    private const int Noon = 12;
    private const int Midnight = 0;

    [Theory]
    [InlineData(0)]
    [InlineData(1)]
    public void ClearAndMostlyClearAreClearByDay(int code)
    {
        Assert.Equal(SkyKind.Clear, SkyCondition.From(code, Noon));
    }

    [Theory]
    [InlineData(2)]
    [InlineData(3)]
    [InlineData(45)]
    [InlineData(48)]
    public void CloudAndFogAreOvercast(int code)
    {
        Assert.Equal(SkyKind.Overcast, SkyCondition.From(code, Noon));
    }

    [Theory]
    [InlineData(51)] [InlineData(55)] [InlineData(57)]
    [InlineData(61)] [InlineData(65)] [InlineData(67)]
    [InlineData(80)] [InlineData(82)]
    public void DrizzleRainAndShowersAllFall(int code)
    {
        Assert.Equal(SkyKind.Rain, SkyCondition.From(code, Noon));
    }

    [Theory]
    [InlineData(71)] [InlineData(75)] [InlineData(77)]
    [InlineData(85)] [InlineData(86)]
    public void SnowAndSnowShowersAreSnow(int code)
    {
        Assert.Equal(SkyKind.Snow, SkyCondition.From(code, Noon));
    }

    [Theory]
    [InlineData(95)]
    [InlineData(99)]
    public void ThunderstormsFallToo(int code)
    {
        Assert.Equal(SkyKind.Rain, SkyCondition.From(code, Noon));
    }

    [Fact]
    public void CodesAboveTheWmoCeilingAreNotThunderstorms()
    {
        // The bug WeatherCodeMap already carried once: an unbounded ">= 95" swallowed every
        // future or garbage code and made the fallback unreachable. 100 is not a wet sky.
        Assert.Equal(SkyKind.Overcast, SkyCondition.From(100, Noon));
        Assert.Equal(SkyKind.Overcast, SkyCondition.From(int.MaxValue, Noon));
    }

    [Fact]
    public void NegativeCodesDoNotFall()
    {
        Assert.Equal(SkyKind.Overcast, SkyCondition.From(-1, Noon));
    }

    [Theory]
    [InlineData(0)]
    [InlineData(1)]
    [InlineData(3)]
    [InlineData(45)]
    public void DrySkiesBecomeNightAfterDark(int code)
    {
        Assert.Equal(SkyKind.Night, SkyCondition.From(code, Midnight));
    }

    [Theory]
    [InlineData(65)]
    [InlineData(75)]
    [InlineData(95)]
    public void WetSkiesIgnoreTheHour(int code)
    {
        // A night gradient with rain streaks over it reads as neither, so rain at night is
        // still drawn as rain.
        Assert.Equal(SkyCondition.From(code, Noon), SkyCondition.From(code, Midnight));
    }

    [Theory]
    [InlineData(0, true)]
    [InlineData(5, true)]
    [InlineData(6, false)]
    [InlineData(19, false)]
    [InlineData(20, true)]
    [InlineData(23, true)]
    public void NightIsTheHoursOutsideTheDayWindow(int hour, bool night)
    {
        Assert.Equal(night, SkyCondition.IsNight(hour));
    }

    [Fact]
    public void TheDayWindowBoundariesBelongToDay()
    {
        // Both ends checked explicitly, because an off-by-one here shows a night sky at six in
        // the morning and nothing about it fails a build.
        Assert.Equal(SkyKind.Clear, SkyCondition.From(0, SkyCondition.DayStartsHour));
        Assert.Equal(SkyKind.Night, SkyCondition.From(0, SkyCondition.NightStartsHour));
        Assert.Equal(SkyKind.Clear, SkyCondition.From(0, SkyCondition.NightStartsHour - 1));
    }
}

public class FirstLookLedgerTests
{
    private static readonly DateOnly Today = new(2026, 9, 10);

    [Fact]
    public void NothingStoredIsAFirstLook()
    {
        // A fresh install, or a settings file that predates the feature.
        Assert.True(FirstLookLedger.IsFirstLook(null, Today));
    }

    [Fact]
    public void YesterdayIsAFirstLook()
    {
        Assert.True(FirstLookLedger.IsFirstLook(Today.AddDays(-1), Today));
    }

    [Fact]
    public void TodayIsNot()
    {
        // The whole point: opening the notch fifty times before noon produces exactly one
        // reveal, not one per open and not one per launch.
        Assert.False(FirstLookLedger.IsFirstLook(Today, Today));
    }

    [Fact]
    public void ADateInTheFutureIsTreatedAsStale()
    {
        // Reachable through a clock correction or a timezone move. Trusting it would suppress
        // the reveal until the calendar caught up, with no way for a person to tell why the
        // animation had stopped happening.
        Assert.True(FirstLookLedger.IsFirstLook(Today.AddDays(3), Today));
    }
}
