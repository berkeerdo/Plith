using Plith.Services;
using Xunit;

namespace Plith.Tests;

/// <summary>
/// The display ranges this was developed against, and the ones it was not.
///
/// The machine here reports 0..100, which makes every conversion the identity and would have let
/// a completely wrong implementation ship looking correct. These cover the ranges no display here
/// has: the coarse panel, the one with a floor it cannot go below, and the one that says its
/// brightness cannot move at all.
/// </summary>
public class BrightnessMathTests
{
    [Theory]
    [InlineData(0u, 0)]
    [InlineData(40u, 40)]
    [InlineData(100u, 100)]
    public void ToPercent_PassesThroughTheCommonRange(uint raw, int expected)
        => Assert.Equal(expected, BrightnessMath.ToPercent(raw, 0, 100));

    [Theory]
    [InlineData(0u, 0)]
    [InlineData(5u, 50)]
    [InlineData(10u, 100)]
    public void ToPercent_ScalesACoarseRange(uint raw, int expected)
        => Assert.Equal(expected, BrightnessMath.ToPercent(raw, 0, 10));

    /// <summary>A panel whose minimum is not zero is saying it cannot go fully dark. The bottom
    /// of its range is therefore 0%, not 20% — reporting the raw number as a percentage would
    /// show a slider that can never reach its own left end.</summary>
    [Theory]
    [InlineData(20u, 0)]
    [InlineData(60u, 50)]
    [InlineData(100u, 100)]
    public void ToPercent_TreatsAFloorAsZero(uint raw, int expected)
        => Assert.Equal(expected, BrightnessMath.ToPercent(raw, 20, 100));

    [Theory]
    [InlineData(150u, 100)]
    [InlineData(5u, 0)]
    public void ToPercent_ClampsAReadingOutsideItsOwnRange(uint raw, int expected)
        => Assert.Equal(expected, BrightnessMath.ToPercent(raw, 20, 100));

    /// <summary>A zero-width range would be a division by zero. It means a display that cannot
    /// change brightness, and the bottom of the range is the only honest answer.</summary>
    [Fact]
    public void ToPercent_SurvivesARangeOfZeroWidth()
        => Assert.Equal(0, BrightnessMath.ToPercent(50, 50, 50));

    [Theory]
    [InlineData(0, 20u)]
    [InlineData(50, 60u)]
    [InlineData(100, 100u)]
    public void ToRaw_PutsAPercentageBackInsideTheRange(int percent, uint expected)
        => Assert.Equal(expected, BrightnessMath.ToRaw(percent, 20, 100));

    [Theory]
    [InlineData(-30, 0u)]
    [InlineData(400, 10u)]
    public void ToRaw_ClampsBeforeScaling(int percent, uint expected)
        => Assert.Equal(expected, BrightnessMath.ToRaw(percent, 0, 10));

    [Fact]
    public void ToRaw_SurvivesARangeOfZeroWidth()
        => Assert.Equal(50u, BrightnessMath.ToRaw(80, 50, 50));

    /// <summary>
    /// A round trip lands on the nearest value the display can actually reach, not on the one it
    /// started from.
    ///
    /// This is the behaviour, not a tolerated error: a panel with eleven steps has no 47%, so
    /// asking for 47 and being told 50 is the display's granularity showing through. Asserting
    /// equality here instead would have been asserting that a coarse display is a fine one.
    /// </summary>
    [Theory]
    [InlineData(47, 50)]
    [InlineData(44, 40)]
    [InlineData(100, 100)]
    [InlineData(0, 0)]
    public void RoundTrip_LandsOnTheNearestReachableStep(int asked, int reachable)
    {
        var raw = BrightnessMath.ToRaw(asked, 0, 10);
        Assert.Equal(reachable, BrightnessMath.ToPercent(raw, 0, 10));
    }

    /// <summary>On a display with a full 0..100 range every percentage is reachable, so there the
    /// round trip IS exact — which is what makes the coarse case above a property of the panel
    /// rather than of the arithmetic.</summary>
    [Theory]
    [InlineData(0)]
    [InlineData(37)]
    [InlineData(99)]
    [InlineData(100)]
    public void RoundTrip_IsExactOnAFineRange(int percent)
        => Assert.Equal(percent, BrightnessMath.ToPercent(BrightnessMath.ToRaw(percent, 0, 100), 0, 100));
}
