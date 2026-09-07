using Plith.Services;

namespace Plith.Tests;

public class WeatherCodeMapTests
{
    // Open-Meteo reports WMO 4677 weather codes. These four are the boundaries of the ranges
    // the map collapses; the codes between them are covered by the range, not by a case each.
    [Theory]
    [InlineData(0)]    // clear sky
    [InlineData(3)]    // overcast
    [InlineData(61)]   // slight rain
    [InlineData(95)]   // thunderstorm
    public void Describe_ReturnsANonEmptyGlyphAndLabelForEveryKnownCode(int code)
    {
        var (glyph, label) = WeatherCodeMap.Describe(code);
        Assert.False(string.IsNullOrWhiteSpace(glyph));
        Assert.False(string.IsNullOrWhiteSpace(label));
    }

    [Fact]
    public void Describe_FallsBackRatherThanThrowingOnAnUnknownCode()
    {
        // The provider can add codes. An exception here would take down the refresh timer on
        // a background thread for a decoration.
        var (glyph, label) = WeatherCodeMap.Describe(4242);
        Assert.False(string.IsNullOrWhiteSpace(glyph));
        Assert.False(string.IsNullOrWhiteSpace(label));
    }

    [Fact]
    public void Describe_DistinguishesClearFromRain()
    {
        Assert.NotEqual(WeatherCodeMap.Describe(0).Label, WeatherCodeMap.Describe(61).Label);
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
