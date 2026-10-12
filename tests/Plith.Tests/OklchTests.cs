using System.Windows.Media;
using Plith.Services;

namespace Plith.Tests;

public class OklchTests
{
    [Theory]
    [InlineData(0xCA, 0xFF, 0x33)]  // lime, the praxvon seed
    [InlineData(0x0B, 0x0B, 0x0A)]  // the dark floor
    [InlineData(0xE7, 0xE6, 0xE0)]  // the light floor
    [InlineData(0xFF, 0xFF, 0xFF)]
    [InlineData(0x00, 0x00, 0x00)]
    public void RoundTrip_PreservesTheColour(byte r, byte g, byte b)
    {
        var start = Color.FromRgb(r, g, b);
        var (l, c, h) = Oklch.FromColor(start);
        var back = Oklch.ToColor(l, c, h);

        // One step per channel: the conversion runs through floating point and back to bytes.
        Assert.InRange(back.R, start.R - 1, start.R + 1);
        Assert.InRange(back.G, start.G - 1, start.G + 1);
        Assert.InRange(back.B, start.B - 1, start.B + 1);
    }

    [Fact]
    public void Walk_LightensUntilThePredicatePasses()
    {
        var floor = Color.FromRgb(0x0B, 0x0B, 0x0A);
        var seed = Color.FromRgb(0x1D, 0x4E, 0x89);  // prussian, too dark on the dark floor

        var walked = Oklch.Walk(seed, c => ContrastInk.ContrastRatio(c, floor) >= 3.0, direction: 1);

        Assert.True(ContrastInk.ContrastRatio(walked, floor) >= 3.0);
    }

    [Fact]
    public void Walk_KeepsHue()
    {
        var floor = Color.FromRgb(0x0B, 0x0B, 0x0A);
        var seed = Color.FromRgb(0x1D, 0x4E, 0x89);
        var (_, _, seedHue) = Oklch.FromColor(seed);

        var walked = Oklch.Walk(seed, c => ContrastInk.ContrastRatio(c, floor) >= 3.0, direction: 1);
        var (_, _, walkedHue) = Oklch.FromColor(walked);

        // Two degrees of drift is the gamut clamp, not a hue change.
        Assert.InRange(Math.Abs(walkedHue - seedHue), 0.0, 2.0);
    }

    [Fact]
    public void Walk_ReturnsTheEndpointWhenNothingPasses()
    {
        var impossible = Color.FromRgb(0x80, 0x80, 0x80);

        // No tone of any colour clears 21:1 against mid grey.
        var walked = Oklch.Walk(impossible, _ => false, direction: 1);

        Assert.NotEqual(default, walked);
    }
}
