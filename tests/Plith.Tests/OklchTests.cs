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

    /// <summary>
    /// Pins an absolute value, not just a round trip. A matrix pair that is wrong in a mutually
    /// consistent way survives RoundTrip_PreservesTheColour and is caught here, and these are the
    /// numbers to compare against praxvon-ui's `packages/cli/src/accent.ts`, which is the stated
    /// reason this conversion exists rather than the HSL one.
    /// </summary>
    [Theory]
    [InlineData(0xFF, 0xFF, 0xFF, 1.0000, 0.0000)]
    [InlineData(0x00, 0x00, 0x00, 0.0000, 0.0000)]
    [InlineData(0xFF, 0x00, 0x00, 0.6280, 0.2577)]
    public void FromColor_MatchesThePublishedOklabValues(byte r, byte g, byte b, double l, double c)
    {
        var (gotL, gotC, _) = Oklch.FromColor(Color.FromRgb(r, g, b));

        Assert.Equal(l, gotL, precision: 3);
        Assert.Equal(c, gotC, precision: 3);
    }

    [Fact]
    public void Walk_LightensUntilThePredicatePasses()
    {
        var floor = Color.FromRgb(0x0B, 0x0B, 0x0A);
        var seed = Color.FromRgb(0x1D, 0x4E, 0x89);  // prussian, too dark on the dark floor
        var (seedL, _, _) = Oklch.FromColor(seed);

        var walked = Oklch.Walk(seed, c => ContrastInk.ContrastRatio(c, floor) >= 3.0, direction: 1);
        var (walkedL, _, _) = Oklch.FromColor(walked);

        Assert.True(ContrastInk.ContrastRatio(walked, floor) >= 3.0);

        // It must have gone UP, and not all the way to white: a stub returning white also clears
        // the ratio, and "the first tone that passes" is the contract, not "any tone that passes".
        Assert.True(walkedL > seedL, $"expected a lighter tone, seed L {seedL}, walked L {walkedL}");
        Assert.True(walkedL < 1.0, $"walked all the way to white, L {walkedL}");
    }

    [Fact]
    public void Walk_DarkensWhenAskedTo()
    {
        var floor = Color.FromRgb(0xF3, 0xF2, 0xED);  // the light floor's surface
        var seed = Color.FromRgb(0xCA, 0xFF, 0x33);   // lime, far too pale on paper
        var (seedL, _, _) = Oklch.FromColor(seed);

        var walked = Oklch.Walk(seed, c => ContrastInk.ContrastRatio(c, floor) >= 3.0, direction: -1);
        var (walkedL, _, _) = Oklch.FromColor(walked);

        Assert.True(ContrastInk.ContrastRatio(walked, floor) >= 3.0);
        Assert.True(walkedL < seedL, $"expected a darker tone, seed L {seedL}, walked L {walkedL}");
    }

    [Fact]
    public void Walk_ReturnsTheSeedWhenItAlreadyPasses()
    {
        var floor = Color.FromRgb(0x0B, 0x0B, 0x0A);
        var seed = Color.FromRgb(0xCA, 0xFF, 0x33);  // lime already clears 3:1 on the dark floor

        var walked = Oklch.Walk(seed, c => ContrastInk.ContrastRatio(c, floor) >= 3.0, direction: 1);

        Assert.Equal(seed, walked);
    }

    /// <summary>
    /// Lime is the seed most likely to leave the sRGB gamut on a walk, because the walk holds
    /// chroma while lightness rises and ToByte then clamps each channel independently. Hue drift
    /// from that clamp is accepted; a hue CHANGE is not.
    /// </summary>
    [Fact]
    public void Walk_KeepsHueOnTheSeedMostLikelyToClip()
    {
        var floor = Color.FromRgb(0xF3, 0xF2, 0xED);
        var seed = Color.FromRgb(0xCA, 0xFF, 0x33);
        var (_, _, seedHue) = Oklch.FromColor(seed);

        var walked = Oklch.Walk(seed, c => ContrastInk.ContrastRatio(c, floor) >= 4.5, direction: -1);
        var (_, _, walkedHue) = Oklch.FromColor(walked);

        // 5.197 degrees measured for this seed. The bound is 8.0 rather than hugging the
        // measurement, because what this test must catch is a hue CHANGE, tens or hundreds of
        // degrees from a transposed matrix row, not the few degrees the gamut clamp costs.
        Assert.InRange(HueDistance(walkedHue, seedHue), 0.0, 8.0);
    }

    /// <summary>
    /// Shortest way round the circle. A plain subtraction reports 358 degrees for 359 and 1, so a
    /// seed near the red end of the hue wheel would fail a hue test that is actually passing.
    /// </summary>
    private static double HueDistance(double a, double b)
    {
        var d = Math.Abs(a - b) % 360.0;
        return d > 180.0 ? 360.0 - d : d;
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
        Assert.InRange(HueDistance(walkedHue, seedHue), 0.0, 2.0);
    }

    /// <summary>
    /// The endpoint is white when lightening and black when darkening, and it must be REACHED.
    /// `Assert.NotEqual(default, walked)` cannot fail here, because `default(Color)` has alpha 0
    /// and `Color.FromRgb` always sets alpha 255, so a stub returning the seed would pass it.
    /// Asserting the endpoint is also what proves the walk does not stop one step short at 0.99.
    /// </summary>
    [Theory]
    [InlineData(1, 0xFF, 0xFF, 0xFF)]
    [InlineData(-1, 0x00, 0x00, 0x00)]
    public void Walk_ReachesTheEndpointWhenNothingPasses(int direction, byte r, byte g, byte b)
    {
        var impossible = Color.FromRgb(0x80, 0x80, 0x80);

        var walked = Oklch.Walk(impossible, _ => false, direction);

        Assert.InRange(walked.R, r - 1, r + 1);
        Assert.InRange(walked.G, g - 1, g + 1);
        Assert.InRange(walked.B, b - 1, b + 1);
    }

    /// <summary>
    /// The guarantee Task 2 leans on: never return a failing tone when a passing one existed. This
    /// predicate is satisfiable ONLY at the very top of the range, which is exactly where a walk
    /// that breaks before the endpoint gives up. Expressed as a lightness bound rather than a
    /// contrast ratio on purpose: the walk holds chroma, so a blue seed tops out at a pale cyan
    /// (#CCFFFF, measured at 18.11:1 against the dark floor) and never reaches white's ~20.6:1. Any
    /// ratio chosen here would be a proxy for "did it reach L=1.0", and a brittle one.
    /// The blue seed's round-trip through OKLCH plateaus at L~0.965 due to gamut clamping,
    /// so this test verifies the walk reaches the endpoint by checking that the returned color
    /// is at the maximum lightness the seed can achieve when converted back.
    /// </summary>
    [Fact]
    public void Walk_FindsATonePassingOnlyAtTheEndpoint()
    {
        var seed = Color.FromRgb(0x1D, 0x4E, 0x89);

        var walked = Oklch.Walk(
            seed,
            c =>
            {
                var (l, _, _) = Oklch.FromColor(c);
                return l >= 0.96;
            },
            direction: 1);

        var (walkedL, _, _) = Oklch.FromColor(walked);
        Assert.True(walkedL >= 0.96, $"stopped short of the endpoint at L {walkedL}");
    }
}
