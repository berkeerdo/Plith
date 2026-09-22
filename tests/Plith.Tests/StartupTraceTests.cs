using Plith.Services;

namespace Plith.Tests;

public class StartupTraceTests
{
    [Fact]
    public void FormatsTheSpansOfACompletedStartup()
    {
        long now = 0;
        var trace = new StartupTrace(() => now, clrMs: 380);

        now = 45; trace.Mark(StartupPhase.App);
        now = 57; trace.Mark(StartupPhase.Settings);
        now = 267; trace.Mark(StartupPhase.Cards);
        now = 427; trace.Mark(StartupPhase.Window);
        now = 522; trace.Mark(StartupPhase.Audio);
        now = 542; trace.Mark(StartupPhase.Hooks);
        now = 602; trace.Mark(StartupPhase.Tray);
        now = 812; trace.Mark(StartupPhase.Shelf);
        now = 860;

        Assert.Equal(
            "Startup: 1240ms total (clr 380, app 45, settings 12, cards 210, window 160, "
            + "audio 95, hooks 20, tray 60, shelf 210, settle 48), max UI stall 640ms over 3 ticks",
            Report(trace, maxStallMs: 640, stallTicks: 3));
    }

    [Fact]
    public void TheSpansPartitionTheTotalRatherThanOverlapping()
    {
        // Ten consecutive spans of one launch, not ten measurements of the same window. A reader
        // adding them up must land on the total, or the table invites double counting and the
        // first column gets blamed twice. This is the same property NotchOpenTrace holds, and it
        // matters more here because there are ten columns rather than four.
        long now = 0;
        var trace = new StartupTrace(() => now, clrMs: 100);

        now = 10; trace.Mark(StartupPhase.App);
        now = 30; trace.Mark(StartupPhase.Settings);
        now = 60; trace.Mark(StartupPhase.Cards);
        now = 100; trace.Mark(StartupPhase.Window);
        now = 150; trace.Mark(StartupPhase.Audio);
        now = 210; trace.Mark(StartupPhase.Hooks);
        now = 280; trace.Mark(StartupPhase.Tray);
        now = 360; trace.Mark(StartupPhase.Shelf);
        now = 400;

        var line = Report(trace, 0, 0);

        // 100 + 10 + 20 + 30 + 40 + 50 + 60 + 70 + 80 + 40 = 500
        Assert.Contains("500ms total (clr 100, app 10, settings 20, cards 30, window 40, "
                        + "audio 50, hooks 60, tray 70, shelf 80, settle 40)", line);
    }

    [Fact]
    public void TheClrSpanIsPartOfTheTotalRatherThanBesideIt()
    {
        // The wait a person feels starts when they double-click the icon, not when managed code
        // gets control. A total that excluded the runtime's own start would be the one number in
        // the line that does not describe their experience.
        long now = 0;
        var trace = new StartupTrace(() => now, clrMs: 700);

        now = 1; trace.Mark(StartupPhase.App);
        now = 2; trace.Mark(StartupPhase.Shelf);
        now = 3;

        Assert.Contains("703ms total", Report(trace, 0, 0));
    }

    [Fact]
    public void APhaseNeverMarkedReadsZeroAndTheNextOneAbsorbsIt()
    {
        // A phase can legitimately not happen: StartShelf returns early when the user's SID
        // cannot be read, and StartBrightness returns early with no settings. Reading zero is
        // correct, and the span it never used has to go somewhere rather than vanish, or the
        // columns stop adding up to the total on exactly the launches that went wrong.
        long now = 0;
        var trace = new StartupTrace(() => now, clrMs: 0);

        now = 10; trace.Mark(StartupPhase.App);
        // Settings, Cards and Window never marked.
        now = 90; trace.Mark(StartupPhase.Audio);
        now = 100;

        var line = Report(trace, 0, 0);

        Assert.Contains("settings 0, cards 0, window 0, audio 80", line);
        Assert.Contains("100ms total", line);
    }

    [Fact]
    public void OnlyTheFirstMarkOfAPhaseCounts()
    {
        // Nothing in App calls Mark twice today, but a phase stamped again later would move its
        // boundary forward and silently steal the following phase's time.
        long now = 0;
        var trace = new StartupTrace(() => now, clrMs: 0);

        now = 10; trace.Mark(StartupPhase.App);
        now = 20; trace.Mark(StartupPhase.Settings);
        now = 900; trace.Mark(StartupPhase.Settings);
        now = 30; // the clock is monotonic in the app; the point is the mark, not the clock
        trace.Mark(StartupPhase.Cards);
        now = 40;

        Assert.Contains("app 10, settings 10, cards 10", Report(trace, 0, 0));
    }

    [Fact]
    public void AMarkArrivingOutOfOrderCannotProduceANegativeSpan()
    {
        // Defensive rather than expected. A negative column would be read as a fast phase rather
        // than as a broken instrument, which is the failure mode this repo has paid for before:
        // an instrument that reports something plausible while measuring nothing.
        long now = 0;
        var trace = new StartupTrace(() => now, clrMs: 0);

        now = 100; trace.Mark(StartupPhase.Cards);
        now = 50; trace.Mark(StartupPhase.App);
        now = 120;

        var line = Report(trace, 0, 0);

        Assert.DoesNotContain("-", line);
        Assert.Contains("120ms total", line);
    }

    [Fact]
    public void TheSettleSpanIsMeasuredAtTheIdleRatherThanAtTheReport()
    {
        // The whole reason the two are separate calls. The line is formatted from the probe's
        // tick, which can be a quarter of a second after the thread actually went idle, and on a
        // launch where the probe ticks late it could be far longer. Measuring the settle at the
        // report would fold that wait into the launch and make every column but one look honest.
        long now = 0;
        var trace = new StartupTrace(() => now, clrMs: 0);

        now = 80; trace.Mark(StartupPhase.Shelf);
        now = 100; trace.Idle();
        now = 5000; // the probe finally ticks

        Assert.Contains("100ms total", trace.Report(0, 1));
    }

    [Fact]
    public void ReportingBeforeTheThreadHasGoneIdleGivesNothing()
    {
        // Input priority sits above ContextIdle, so a probe tick CAN arrive before the settle.
        // Nothing is lost when it does: the probe keeps ticking and the next one reports.
        long now = 0;
        var trace = new StartupTrace(() => now, clrMs: 0);

        now = 10; trace.Mark(StartupPhase.Shelf);

        Assert.Null(trace.Report(0, 1));

        now = 20; trace.Idle();
        now = 30;

        Assert.NotNull(trace.Report(700, 2));
    }

    [Fact]
    public void ReportingTwiceGivesNothingTheSecondTime()
    {
        // The probe ticks for the life of the process, so Report is called four times a second
        // forever. Only the first call may produce a line; a second would read as a second launch.
        long now = 0;
        var trace = new StartupTrace(() => now, clrMs: 0);

        now = 10; trace.Mark(StartupPhase.Shelf);
        now = 20;

        Assert.NotNull(Report(trace, 0, 0));
        Assert.Null(Report(trace, 0, 0));
    }

    [Fact]
    public void TheStallIsReportedWithItsTickCount()
    {
        // A stall of zero has two completely different meanings: nothing blocked, or nothing was
        // ever sampled. Without the count the second reads as the first. The same rule
        // NotchOpenTrace records, and the reason section 5 of PERF-VERIFICATION could say plainly
        // that its own stall column said nothing.
        long now = 0;
        var trace = new StartupTrace(() => now, clrMs: 0);

        now = 10; trace.Mark(StartupPhase.Shelf);
        now = 20;

        Assert.Contains("max UI stall 0ms over 0 ticks", Report(trace, 0, 0));
    }

    /// <summary>The two calls App makes, in the order it makes them: the settle stamps the end of
    /// the launch, and the probe's tick formats the line once there is a stall to put in it.</summary>
    private static string? Report(StartupTrace trace, long maxStallMs, int stallTicks)
    {
        trace.Idle();
        return trace.Report(maxStallMs, stallTicks);
    }
}
