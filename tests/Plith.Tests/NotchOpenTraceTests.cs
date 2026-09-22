using Plith.Services;

namespace Plith.Tests;

public class NotchOpenTraceTests
{
    [Fact]
    public void FormatsTheIntervalsOfACompletedOpen()
    {
        long now = 0;
        var trace = new NotchOpenTrace(() => now);

        trace.Click();
        now = 19; trace.LaidOut();
        now = 20; trace.Deferred();
        now = 22; trace.Shown();
        now = 30;

        Assert.Equal("Open #1: 30ms total (layout 19, defer 1, show 2, settle 8), max UI stall 0ms over 0 ticks",
                     trace.Settled());
    }

    [Fact]
    public void TheIntervalsPartitionTheTotalRatherThanOverlapping()
    {
        // layout, defer, show and settle are consecutive spans of one open, not four
        // measurements of the same window. A reader adding them up must land on the total, or
        // the table invites double counting and the first column gets blamed twice.
        long now = 0;
        var trace = new NotchOpenTrace(() => now);

        trace.Click();
        now = 100; trace.LaidOut();
        now = 130; trace.Deferred();
        now = 145; trace.Shown();
        now = 200;

        var line = trace.Settled();
        Assert.Contains("200ms total (layout 100, defer 30, show 15, settle 55)", line);
    }

    [Fact]
    public void OnlyTheFirstLayoutPassOfAnOpenCounts()
    {
        // The handler is attached at the click and detached on the first pass, but the expansion
        // animates and a later pass would otherwise overwrite the stamp with a time that has
        // nothing to do with laying the pages out.
        long now = 0;
        var trace = new NotchOpenTrace(() => now);

        trace.Click();
        now = 19; trace.LaidOut();
        now = 300; trace.LaidOut();
        now = 310; trace.Deferred();
        trace.Shown();
        now = 320;

        Assert.Contains("layout 19", trace.Settled());
    }

    [Fact]
    public void AnOpenWithNoLayoutPassReportsZeroLayout()
    {
        // Not hypothetical: the pages are laid out once and stay laid out, so every open after
        // the first can legitimately produce no layout pass at all. That must read as zero
        // rather than as the whole wait, which is what an unset stamp would produce.
        long now = 0;
        var trace = new NotchOpenTrace(() => now);

        trace.Click();
        now = 5; trace.Deferred();
        now = 7; trace.Shown();
        now = 9;

        Assert.Contains("(layout 0, defer 5, show 2, settle 2)", trace.Settled());
    }

    [Fact]
    public void SettledReportsOnlyOncePerOpen()
    {
        long now = 0;
        var trace = new NotchOpenTrace(() => now);
        trace.Click();
        trace.Deferred();
        trace.Shown();

        now = 40;
        Assert.NotNull(trace.Settled());

        now = 56;
        Assert.Null(trace.Settled());
    }

    [Fact]
    public void SettledReturnsNullWhenNoOpenIsInFlight()
    {
        long now = 0;
        var trace = new NotchOpenTrace(() => now);

        Assert.Null(trace.Settled());
    }

    [Fact]
    public void AStraySettleDoesNotConsumeAnOrdinal()
    {
        // The whole point of this instrument is that the FIRST open is the one under suspicion.
        // A stray callback taking number one would make the real first open report as the second.
        long now = 0;
        var trace = new NotchOpenTrace(() => now);

        trace.Settled();

        now = 100; trace.Click();
        now = 110; trace.Deferred();
        now = 112; trace.Shown();
        now = 200;

        Assert.StartsWith("Open #1:", trace.Settled());
    }

    [Fact]
    public void MaxStallIsTheLargestGapBetweenProbeTicks()
    {
        // The probe ticks at Input priority. A UI thread that cannot pump input cannot tick it
        // either, so the gap between two ticks IS the block the busy cursor was showing. This
        // is the only figure here that measures the reported symptom rather than the duration
        // of the open, and the two are not the same: the expansion is a deliberate 340 ms
        // animation and is not a hang.
        long now = 0;
        var trace = new NotchOpenTrace(() => now);
        trace.Click();

        now = 16; trace.Tick();
        now = 32; trace.Tick();
        now = 750; trace.Tick();
        now = 766; trace.Tick();

        trace.Deferred();
        trace.Shown();
        now = 800;

        Assert.Contains("max UI stall 718ms over 4 ticks", trace.Settled());
    }

    [Fact]
    public void ReportsHowOftenTheProbeSampled()
    {
        // A stall of zero has two completely different meanings: nothing blocked, or nothing
        // was ever sampled because the open was shorter than the probe interval. Without the
        // count the second reads as the first, and the instrument quietly reports "no block"
        // for a window it never looked at.
        long now = 0;
        var trace = new NotchOpenTrace(() => now);
        trace.Click();
        trace.Deferred();
        trace.Shown();
        now = 8;

        Assert.Contains("max UI stall 0ms over 0 ticks", trace.Settled());
    }

    [Fact]
    public void StallIsMeasuredFromTheClickSoAnImmediateBlockIsSeen()
    {
        // If the thread blocks the instant the click lands, no tick arrives until it ends, so
        // there is no earlier tick to measure a gap from. Anchoring at the click is what makes
        // that case visible instead of silent.
        long now = 0;
        var trace = new NotchOpenTrace(() => now);
        trace.Click();

        now = 900; trace.Tick();

        trace.Deferred();
        trace.Shown();
        now = 920;

        Assert.Contains("max UI stall 900ms over 1 ticks", trace.Settled());
    }

    [Fact]
    public void StallDoesNotCarryOverToTheNextOpen()
    {
        long now = 0;
        var trace = new NotchOpenTrace(() => now);
        trace.Click();
        now = 900; trace.Tick();
        trace.Deferred();
        trace.Shown();
        now = 920; trace.Settled();

        now = 1000; trace.Click();
        now = 1016; trace.Tick();
        trace.Deferred();
        trace.Shown();
        now = 1030;

        Assert.Contains("max UI stall 16ms over 1 ticks", trace.Settled());
    }

    [Fact]
    public void TicksArrivingOutsideAnOpenAreIgnored()
    {
        // Settled() disarms the trace, but the probe timer is stopped by the caller a moment
        // later, so a tick can still land in between. It must not become the next open's stall.
        long now = 0;
        var trace = new NotchOpenTrace(() => now);
        trace.Click();
        trace.Deferred();
        trace.Shown();
        now = 100; trace.Settled();

        now = 5000; trace.Tick();

        now = 6000; trace.Click();
        now = 6016; trace.Tick();
        trace.Deferred();
        trace.Shown();
        now = 6030;

        Assert.Contains("max UI stall 16ms over 1 ticks", trace.Settled());
    }

    [Fact]
    public void ALayoutPassOutsideAnOpenIsIgnored()
    {
        long now = 0;
        var trace = new NotchOpenTrace(() => now);

        now = 500; trace.LaidOut();

        now = 1000; trace.Click();
        now = 1020; trace.LaidOut();
        trace.Deferred();
        trace.Shown();
        now = 1030;

        Assert.Contains("layout 20", trace.Settled());
    }
}
