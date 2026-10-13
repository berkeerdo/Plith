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
    /// The endpoint must be TRIED, and this is the regression test for the walk that stopped one
    /// 0.01 step short of it. It is also the guarantee Task 2 leans on: never return a failing
    /// tone when a passing one existed.
    ///
    /// Three earlier shapes of this test did not work, each for a reason worth keeping:
    ///
    /// 1. `Assert.NotEqual(default, walked)` cannot fail at all. `default(Color)` has alpha 0 and
    ///    `Color.FromRgb` always sets 255, so a stub returning the seed passes it.
    /// 2. Asserting a round-tripped lightness cannot work either, because the walk holds chroma
    ///    and `ToColor` clamps out-of-gamut channels, so lightness COMPRESSES near the top:
    ///    for the seed below, the last short step round-trips to L 0.9638 and the true endpoint to
    ///    L 0.9650. Any threshold has to live in that 0.0012 window. Measured, not reasoned.
    /// 3. A grey seed does not discriminate. Mid grey sits at L 0.5999, so its step grid lands on
    ///    0.9999, which already renders #FFFFFF: the broken walk and the fixed one return the
    ///    identical colour. The seed must carry chroma for the two to be different colours.
    ///
    /// So: a chromatic seed, and exact equality against the endpoint colour computed from that
    /// seed's own chroma and hue. No threshold and no tolerance, nothing to tune. With the short
    /// step the walk returns #CAFFFF and the assertion fails; with the endpoint tried it returns
    /// #CCFFFF and passes.
    /// </summary>
    [Theory]
    [InlineData(0x1D, 0x4E, 0x89, 1)]   // prussian lightening: #CAFFFF short, #CCFFFF at the end
    [InlineData(0xFF, 0x00, 0x00, -1)]  // red darkening: #130000 short, #100000 at the end
    public void Walk_TriesTheExactEndpointWhenNothingPasses(byte r, byte g, byte b, int direction)
    {
        var seed = Color.FromRgb(r, g, b);
        var (_, c, h) = Oklch.FromColor(seed);
        var endpoint = Oklch.ToColor(direction > 0 ? 1.0 : 0.0, c, h);

        var walked = Oklch.Walk(seed, _ => false, direction);

        Assert.Equal(endpoint, walked);
    }
}
