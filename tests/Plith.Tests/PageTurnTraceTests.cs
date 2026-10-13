using Plith.Services;

namespace Plith.Tests;

public class PageTurnTraceTests
{
    [Fact]
    public void FormatsACompletedTurn()
    {
        long now = 0;
        var trace = new PageTurnTrace(() => now);

        trace.Begin(3, 4, 5, 260);
        now = 16_000; trace.Frame(14.0);
        now = 32_000; trace.Frame(10.0);
        now = 48_000; trace.Frame(6.0);
        now = 64_000; trace.Frame(2.0);

        Assert.Equal("Turn #1: 3->4 of 5 in 260ms: 4 frames, median 16.0ms, worst 16.0ms, " +
                     "0 late; travel 12.00 DIP, biggest step 4.00, 0 still",
                     trace.Settled());
    }

    [Fact]
    public void TheFirstIntervalIsMeasuredFromTheStartOfTheTurn()
    {
        // A slide whose FIRST frame arrives late is the starved case this instrument exists to
        // see, and it is invisible if intervals are only measured between frames: there is no
        // earlier frame to measure the gap from. Anchoring at Begin is the same choice
        // NotchOpenTrace made for its stall, and for the same reason.
        long now = 0;
        var trace = new PageTurnTrace(() => now);

        trace.Begin(0, 1, 5, 260);
        now = 120_000; trace.Frame(14.0);
        now = 136_000; trace.Frame(7.0);

        Assert.Contains("worst 120.0ms", trace.Settled());
    }

    [Fact]
    public void MedianIgnoresASingleOutlierThatTheMeanWouldHide()
    {
        // The median is the baseline every other column is read against, so it has to describe
        // the frames that arrived normally. One 200 ms stall in a 260 ms slide would drag a mean
        // far enough that the stall itself would no longer count as late against it, and the
        // instrument would report a smooth turn with one slow frame as entirely ordinary.
        long now = 0;
        var trace = new PageTurnTrace(() => now);

        trace.Begin(0, 1, 5, 260);
        now = 16_000; trace.Frame(14.0);
        now = 32_000; trace.Frame(12.0);
        now = 232_000; trace.Frame(8.0);
        now = 248_000; trace.Frame(4.0);

        var line = trace.Settled();
        Assert.Contains("median 16.0ms", line);
        Assert.Contains("worst 200.0ms", line);
    }

    [Fact]
    public void CountsFramesThatArrivedAtLeastHalfAgainLateAsLate()
    {
        // 1.5x the median is the threshold for a frame a person can see: at 60 Hz it means a
        // frame that took 25 ms instead of 16.6, which is one refresh missed. Counted rather
        // than only reported as a worst case, because one hitch in a slide and five hitches in
        // the same slide are different defects and the worst column cannot tell them apart.
        long now = 0;
        var trace = new PageTurnTrace(() => now);

        trace.Begin(0, 1, 5, 260);
        now = 16_000; trace.Frame(14.0);
        now = 32_000; trace.Frame(12.0);
        now = 48_000; trace.Frame(10.0);
        now = 73_000; trace.Frame(8.0);   // 25 ms, late
        now = 89_000; trace.Frame(6.0);
        now = 123_000; trace.Frame(4.0);  // 34 ms, late

        Assert.Contains("2 late", trace.Settled());
    }

    [Fact]
    public void CountsFramesWhoseValueDidNotMoveAsStill()
    {
        // THE SECOND AXIS, and the whole reason the value travels with the clock. A frame that
        // arrived on time carrying the same position as the one before it is a frame a person
        // sees as a pause, and every timing column above would call that turn perfect. The
        // slide is only 14 DIP under an ease-out, so its last frames move a fraction of a pixel
        // and this is where quantization would show up if it is there.
        long now = 0;
        var trace = new PageTurnTrace(() => now);

        trace.Begin(0, 1, 5, 260);
        now = 16_000; trace.Frame(14.0);
        now = 32_000; trace.Frame(10.0);
        now = 48_000; trace.Frame(10.0);
        now = 64_000; trace.Frame(10.0);
        now = 80_000; trace.Frame(6.0);

        Assert.Contains("2 still", trace.Settled());
    }

    [Fact]
    public void TravelIsTheDistanceFromTheFirstFrameToTheLast()
    {
        long now = 0;
        var trace = new PageTurnTrace(() => now);

        trace.Begin(0, 1, 5, 260);
        now = 16_000; trace.Frame(-14.0);
        now = 32_000; trace.Frame(-8.5);
        now = 48_000; trace.Frame(0.0);

        Assert.Contains("travel 14.00 DIP", trace.Settled());
    }

    [Fact]
    public void BiggestStepIsTheLargestDistanceCoveredBetweenTwoFrames()
    {
        long now = 0;
        var trace = new PageTurnTrace(() => now);

        trace.Begin(0, 1, 5, 260);
        now = 16_000; trace.Frame(14.0);
        now = 32_000; trace.Frame(6.0);
        now = 48_000; trace.Frame(5.0);

        Assert.Contains("biggest step 8.00", trace.Settled());
    }

    [Fact]
    public void ATurnThatRenderedNoFrameAtAllSaysSoRatherThanReportingZeros()
    {
        // Not hypothetical, and it must not read as a smooth turn. A slide that produced no
        // composition tick is the worst outcome this instrument can find, and a line reading
        // "median 0.0ms, 0 late" would be indistinguishable from a perfect one.
        long now = 0;
        var trace = new PageTurnTrace(() => now);

        trace.Begin(2, 3, 5, 260);
        now = 260_000;

        Assert.Equal("Turn #1: 2->3 of 5 in 260ms: no frames rendered", trace.Settled());
    }

    [Fact]
    public void ASingleFrameHasNoStepToReport()
    {
        // One frame yields one interval (from Begin) and no step, because a step needs two
        // positions. Reporting a step of zero would claim the page held still when nothing was
        // ever measured.
        long now = 0;
        var trace = new PageTurnTrace(() => now);

        trace.Begin(0, 1, 5, 260);
        now = 16_000; trace.Frame(14.0);

        var line = trace.Settled();
        Assert.Contains("1 frames", line);
        Assert.Contains("travel 0.00 DIP, biggest step 0.00, 0 still", line);
    }

    [Fact]
    public void SettledReturnsNullWhenNoTurnIsInFlight()
    {
        long now = 0;
        var trace = new PageTurnTrace(() => now);

        Assert.Null(trace.Settled());
    }

    [Fact]
    public void SettledReportsOnlyOncePerTurn()
    {
        long now = 0;
        var trace = new PageTurnTrace(() => now);

        trace.Begin(0, 1, 5, 260);
        now = 16_000; trace.Frame(14.0);
        now = 260_000;

        Assert.NotNull(trace.Settled());
        Assert.Null(trace.Settled());
    }

    [Fact]
    public void AFrameArrivingOutsideATurnIsIgnored()
    {
        // The Rendering handler is detached by the caller a moment after the settle, so a frame
        // can still land in between. It must not become the next turn's first sample, which
        // would report a step the next slide never made.
        long now = 0;
        var trace = new PageTurnTrace(() => now);

        trace.Begin(0, 1, 5, 260);
        now = 16_000; trace.Frame(14.0);
        now = 260_000; trace.Settled();

        now = 300_000; trace.Frame(99.0);

        now = 400_000; trace.Begin(1, 2, 5, 260);
        now = 416_000; trace.Frame(14.0);
        now = 432_000; trace.Frame(10.0);

        Assert.Contains("travel 4.00 DIP", trace.Settled());
    }

    [Fact]
    public void NothingCarriesOverFromOneTurnToTheNext()
    {
        long now = 0;
        var trace = new PageTurnTrace(() => now);

        trace.Begin(0, 1, 5, 260);
        now = 200_000; trace.Frame(14.0);
        now = 216_000; trace.Frame(0.0);
        trace.Settled();

        now = 400_000; trace.Begin(1, 2, 5, 260);
        now = 416_000; trace.Frame(14.0);
        now = 432_000; trace.Frame(12.0);

        var line = trace.Settled();
        Assert.Contains("worst 16.0ms", line);
        Assert.Contains("0 late", line);
        Assert.Contains("biggest step 2.00", line);
    }

    [Fact]
    public void TheOrdinalCountsTurnsThatCompletedRatherThanTurnsThatStarted()
    {
        // A turn abandoned before it settled must not consume a number. The comparison this
        // instrument is built for sets the turn onto the shelf beside the four ordinary ones in
        // the same run, and a reader matching log lines to gestures needs the count to mean
        // what it says.
        long now = 0;
        var trace = new PageTurnTrace(() => now);

        trace.Begin(0, 1, 5, 260);
        now = 16_000; trace.Frame(14.0);
        now = 260_000; trace.Settled();

        now = 400_000; trace.Begin(1, 2, 5, 260);
        now = 416_000; trace.Frame(14.0);

        Assert.StartsWith("Turn #2:", trace.Settled());
    }

    [Fact]
    public void BeginDiscardsATurnThatNeverSettled()
    {
        // Every path that starts a slide calls Begin, and a slide can be replaced by the next
        // one before its window is out: ShowCurrentPage removes an outgoing page the moment a
        // new turn starts. The samples of the turn that was cut off belong to neither line.
        long now = 0;
        var trace = new PageTurnTrace(() => now);

        trace.Begin(0, 1, 5, 260);
        now = 16_000; trace.Frame(14.0);
        now = 32_000; trace.Frame(2.0);

        now = 100_000; trace.Begin(1, 2, 5, 260);
        now = 116_000; trace.Frame(14.0);
        now = 132_000; trace.Frame(12.0);

        var line = trace.Settled();
        Assert.StartsWith("Turn #1: 1->2 of 5", line);
        Assert.Contains("2 frames", line);
        Assert.Contains("biggest step 2.00", line);
    }
}
