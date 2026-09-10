using Plith.Services;

namespace Plith.Tests;

public class MarqueeDecisionTests
{
    [Fact]
    public void TextNarrowerThanTheViewportDoesNotScroll()
    {
        Assert.False(MarqueeDecision.Plan(120, 200, reducedMotion: false).Scroll);
    }

    [Fact]
    public void TextExactlyTheViewportWidthDoesNotScroll()
    {
        Assert.False(MarqueeDecision.Plan(200, 200, reducedMotion: false).Scroll);
    }

    [Fact]
    public void SubPixelOverflowDoesNotScroll()
    {
        // Measurement and layout rounding disagree by fractions of a pixel routinely. Without a
        // tolerance a title that visually fits would shimmer forever for no visible reason.
        Assert.False(MarqueeDecision.Plan(200.4, 200, reducedMotion: false).Scroll);
    }

    [Fact]
    public void OverflowScrollsByExactlyTheOverflow()
    {
        var plan = MarqueeDecision.Plan(320, 200, reducedMotion: false);

        Assert.True(plan.Scroll);
        Assert.Equal(-120, plan.ShiftDip, 5);
    }

    [Fact]
    public void SpeedIsConstantRatherThanDuration()
    {
        // The whole rule. A fixed duration would make these two move at wildly different speeds;
        // a fixed speed makes the longer one take proportionally longer.
        var small = MarqueeDecision.Plan(200 + 140, 200, reducedMotion: false);
        var large = MarqueeDecision.Plan(200 + 280, 200, reducedMotion: false);

        Assert.Equal(140 / MarqueeDecision.SpeedDipPerSecond, small.Duration.TotalSeconds, 4);
        Assert.Equal(280 / MarqueeDecision.SpeedDipPerSecond, large.Duration.TotalSeconds, 4);
        Assert.Equal(2 * small.Duration.TotalSeconds, large.Duration.TotalSeconds, 4);
    }

    [Fact]
    public void ShortOverflowsAreHeldAtTheDurationFloor()
    {
        // Four pixels at 28 dip/s is a seventh of a second, which reads as a twitch. The floor
        // turns it into a slow drift instead.
        var plan = MarqueeDecision.Plan(204, 200, reducedMotion: false);

        Assert.True(plan.Scroll);
        Assert.Equal(MarqueeDecision.MinDuration, plan.Duration);
    }

    [Fact]
    public void ReducedMotionNeverScrolls()
    {
        var plan = MarqueeDecision.Plan(900, 100, reducedMotion: true);

        Assert.False(plan.Scroll);
        Assert.Equal(0, plan.ShiftDip);
        Assert.Equal(TimeSpan.Zero, plan.Duration);
    }

    [Fact]
    public void AnUnmeasuredViewportDoesNotStartEveryTitleScrolling()
    {
        // Asked during the first layout pass, before a width has been assigned. "Everything
        // overflows an empty viewport" would set every title moving on load.
        Assert.False(MarqueeDecision.Plan(300, 0, reducedMotion: false).Scroll);
        Assert.False(MarqueeDecision.Plan(300, -5, reducedMotion: false).Scroll);
    }

    [Fact]
    public void EmptyTextDoesNotScroll()
    {
        Assert.False(MarqueeDecision.Plan(0, 200, reducedMotion: false).Scroll);
    }

    [Fact]
    public void NotScrollingCarriesNoShiftOrDuration()
    {
        var plan = MarqueeDecision.Plan(100, 200, reducedMotion: false);

        Assert.Equal(0, plan.ShiftDip);
        Assert.Equal(TimeSpan.Zero, plan.Duration);
    }

    [Fact]
    public void TheShiftIsNegativeSoTheTailIsRevealed()
    {
        Assert.True(MarqueeDecision.Plan(400, 200, reducedMotion: false).ShiftDip < 0);
    }
}
