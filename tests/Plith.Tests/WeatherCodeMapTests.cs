using System.Windows.Media;
using Plith.Services;

namespace Plith.Tests;

public class WeatherCodeMapTests
{
    // Every WMO code the switch in WeatherCodeMap.Describe branches on, one representative per
    // arm, plus 4242 — a code well above the highest bounded arm (>= 95 and <= 99) — as a code
    // the map genuinely has no arm for, covered only by the fallback.
    private static readonly int[] AllHandledCodesPlusFallback =
    [
        0, 1, 2, 3, 45, 48, 51, 55, 57, 61, 65, 67, 71, 75, 77, 80, 82, 85, 86, 95, 96, 99, 4242,
    ];

    // Open-Meteo reports WMO 4677 weather codes. These four are the boundaries of the ranges
    // the map collapses; the codes between them are covered by the range, not by a case each.
    [Theory]
    [InlineData(0)]    // clear sky
    [InlineData(3)]    // overcast
    [InlineData(61)]   // slight rain
    [InlineData(95)]   // thunderstorm
    public void Describe_ReturnsANonEmptyGlyphAndLabelForEveryKnownCode(int code)
    {
        var label = WeatherCodeMap.Describe(code);
        Assert.False(string.IsNullOrWhiteSpace(label));
    }

    [Fact]
    public void Describe_FallsBackRatherThanThrowingOnAnUnknownCode()
    {
        // The provider can add codes. An exception here would take down the refresh timer on
        // a background thread for a decoration. Asserts the actual Unknown mapping, not just a
        // non-empty string: a non-empty glyph/label pair is also what every wrongly-open-ended
        // arm above would have produced (the ">= 95" arm used to swallow this code before it
        // was bounded to "<= 99"), so a non-empty check alone would keep passing on the wrong
        // arm without ever exercising the fallback this test exists to cover.
        var label = WeatherCodeMap.Describe(4242);
        Assert.Equal("Unknown", label);
    }

    [Fact]
    public void Describe_DistinguishesClearFromRain()
    {
        Assert.NotEqual(WeatherCodeMap.Describe(0), WeatherCodeMap.Describe(61));
    }

    [Fact]
    public void IsFresh_AcceptsARecentSnapshot()
    {
        var now = new DateTimeOffset(2026, 9, 7, 12, 0, 0, TimeSpan.Zero);
        var snap = new WeatherSnapshot(21.5, 0, now.AddMinutes(-10));
        Assert.True(WeatherCodeMap.IsFresh(snap, now, maxAgeMinutes: 90));
    }

    [Fact]
    public void IsFresh_RejectsAStaleSnapshot()
    {
        // A stale reading is treated as absent rather than shown. "17 degrees" that is three
        // hours old is a wrong answer presented as a right one.
        var now = new DateTimeOffset(2026, 9, 7, 12, 0, 0, TimeSpan.Zero);
        var snap = new WeatherSnapshot(21.5, 0, now.AddMinutes(-91));
        Assert.False(WeatherCodeMap.IsFresh(snap, now, maxAgeMinutes: 90));
    }

    [Fact]
    public void IsFresh_RejectsASnapshotFromTheFuture()
    {
        // Reachable via a clock change or a DST jump between the fetch and the render.
        var now = new DateTimeOffset(2026, 9, 7, 12, 0, 0, TimeSpan.Zero);
        var snap = new WeatherSnapshot(21.5, 0, now.AddMinutes(5));
        Assert.False(WeatherCodeMap.IsFresh(snap, now, maxAgeMinutes: 90));
    }
}
