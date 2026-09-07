using System.Windows.Media;
using Plith.Services;

namespace Plith.Tests;

public class WeatherCodeMapTests
{
    // Every WMO code the switch in WeatherCodeMap.Describe branches on, one representative per
    // arm, plus a code the map has no arm for at all (covered by the fallback).
    private static readonly int[] AllHandledCodesPlusFallback =
    [
        0, 1, 2, 3, 45, 48, 51, 55, 57, 61, 65, 67, 71, 75, 77, 80, 82, 85, 86, 95, 96, 99, 4242,
    ];

    // Regression test for a Critical defect: seven of the ten original glyph choices were code
    // points that do not exist in Segoe MDL2 Assets at all (an undefined gap between the real
    // Frigid and Unknown glyphs), so they rendered as a hollow "tofu" box on a real Windows
    // build. The unit tests above only assert the glyph string is non-empty, which every one of
    // those broken code points also satisfies — a non-empty string is not a real glyph. This
    // test asks the font itself, via GlyphTypeface.CharacterToGlyphMap, whether the code point
    // Describe returns is an actual defined glyph, which is the only check that would have
    // caught the original defect.
    [Fact]
    public void Describe_EveryGlyphExistsInTheDeclaredFont()
    {
        var glyphTypeface = new GlyphTypeface(new Uri(WeatherCodeMap.GlyphFontFilePath));

        foreach (var code in AllHandledCodesPlusFallback)
        {
            var (glyph, label) = WeatherCodeMap.Describe(code);
            Assert.True(glyph.Length == 1, $"Glyph for code {code} ({label}) is not a single UTF-16 code unit: \"{glyph}\".");
            Assert.True(
                glyphTypeface.CharacterToGlyphMap.ContainsKey(glyph[0]),
                $"Glyph U+{(int)glyph[0]:X4} for code {code} ({label}) is not a defined glyph in " +
                $"{WeatherCodeMap.GlyphFontFamilyName} ({WeatherCodeMap.GlyphFontFilePath}) — it will render as a hollow box.");
        }
    }

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
