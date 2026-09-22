using Plith.Services;

namespace Plith.Tests;

public class UiStallWatchTests
{
    [Fact]
    public void AGapUnderTheThresholdReportsNothing()
    {
        long now = 0;
        var watch = new UiStallWatch(() => now, thresholdMs: 500);

        now = 250; Assert.Null(watch.Tick());
        now = 400; Assert.Null(watch.Tick());
    }

    [Fact]
    public void AGapOverTheThresholdReportsIt()
    {
        long now = 0;
        var watch = new UiStallWatch(() => now, thresholdMs: 500);

        now = 900;

        Assert.Equal("UI thread stalled 900 ms", watch.Tick());
    }

    [Fact]
    public void AGapIsReportedOnceRatherThanOnEveryTickAfterIt()
    {
        // The gap is measured between consecutive ticks, so a block that has ended is over. A
        // watch that kept reporting it would fill the log with one event repeated.
        long now = 0;
        var watch = new UiStallWatch(() => now, thresholdMs: 500);

        now = 900; Assert.NotNull(watch.Tick());
        now = 1150; Assert.Null(watch.Tick());
    }

    [Fact]
    public void TheFirstGapIsMeasuredFromTheAnchorRatherThanFromTheFirstTick()
    {
        // The worst case of all is a thread that blocks immediately and ticks nothing until the
        // block ends. Anchoring on the first tick would measure that as zero, which is the one
        // reading that must never be produced: the total hang, reported as no hang at all.
        long now = 500;
        var watch = new UiStallWatch(() => now, thresholdMs: 500);

        now = 1800;

        Assert.Equal("UI thread stalled 1300 ms", watch.Tick());
    }

    [Fact]
    public void TheLargestGapAndTheTickCountAreKept()
    {
        // These two are what StartupTrace's stall column is built from, so the watch has to hold
        // them rather than only report exceedances as they happen.
        long now = 0;
        var watch = new UiStallWatch(() => now, thresholdMs: 500);

        now = 100; watch.Tick();
        now = 800; watch.Tick();
        now = 900; watch.Tick();

        Assert.Equal(700, watch.MaxGapMs);
        Assert.Equal(3, watch.Ticks);
    }

    [Fact]
    public void ItHasNoNotionOfStartupHavingFinished()
    {
        // The reason this is a separate type from StartupTrace. The reported second could not be
        // reproduced on demand, so the watch outlives the launch it was armed during and keeps
        // reporting for the life of the process.
        long now = 0;
        var watch = new UiStallWatch(() => now, thresholdMs: 500);

        now = 100; watch.Tick();
        now = 3_600_000; // an hour later

        Assert.NotNull(watch.Tick());
    }

    [Fact]
    public void AGapExactlyAtTheThresholdIsNotAStall()
    {
        // The threshold is the largest gap considered ordinary, and the probe's own interval plus
        // scheduling slop lives under it. Reporting the boundary itself would make the log noisy
        // with the instrument's own jitter.
        long now = 0;
        var watch = new UiStallWatch(() => now, thresholdMs: 500);

        now = 500; Assert.Null(watch.Tick());
        now = 1001; Assert.NotNull(watch.Tick());
    }
}
