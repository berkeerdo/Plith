using Plith.Services;

namespace Plith.Tests;

public class StartupWindowTraceTests
{
    [Fact]
    public void FormatsTheSpansInsideTheWindowPhase()
    {
        long now = 200;   // not zero: the phase starts partway into a launch, never at its start
        var trace = new StartupWindowTrace(() => now);

        now = 212; trace.Mark(StartupWindowPhase.Fields);
        now = 213; trace.Mark(StartupWindowPhase.Shell);
        now = 215; trace.Mark(StartupWindowPhase.Band);
        now = 264; trace.Mark(StartupWindowPhase.Content);
        now = 273; trace.Mark(StartupWindowPhase.Pages);
        now = 275; trace.Mark(StartupWindowPhase.Accent);
        now = 379; trace.Mark(StartupWindowPhase.Hwnd);
        now = 540; trace.Mark(StartupWindowPhase.Present);
        now = 546; trace.Mark(StartupWindowPhase.Weather);   // Host skipped below, on purpose

        Assert.Equal(
            "Window: 346ms total (fields 12, shell 1, band 2, content 49, pages 9, accent 2, "
            + "hwnd 104, present 161, host 0, weather 6)",
            trace.Report());
    }

    [Fact]
    public void TheSpansPartitionTheTotalRatherThanOverlapping()
    {
        // Ten consecutive spans of one phase, not ten measurements of the same window. A
        // reader adding them up must land on the total, or the table invites double counting and
        // the largest column gets blamed twice. This is the whole reason the phase is worth
        // splitting at all: the answer wanted is "which of these ten", and an overlapping
        // instrument cannot give it.
        long now = 1000;
        var trace = new StartupWindowTrace(() => now);

        now = 1010; trace.Mark(StartupWindowPhase.Fields);
        now = 1030; trace.Mark(StartupWindowPhase.Shell);
        now = 1060; trace.Mark(StartupWindowPhase.Band);
        now = 1100; trace.Mark(StartupWindowPhase.Content);
        now = 1150; trace.Mark(StartupWindowPhase.Pages);
        now = 1210; trace.Mark(StartupWindowPhase.Accent);
        now = 1280; trace.Mark(StartupWindowPhase.Hwnd);
        now = 1360; trace.Mark(StartupWindowPhase.Present);
        now = 1450; trace.Mark(StartupWindowPhase.Host);
        now = 1550; trace.Mark(StartupWindowPhase.Weather);

        // 10 + 20 + 30 + 40 + 50 + 60 + 70 + 80 + 90 + 100 = 550
        Assert.Equal(
            "Window: 550ms total (fields 10, shell 20, band 30, content 40, pages 50, accent 60, "
            + "hwnd 70, present 80, host 90, weather 100)",
            trace.Report());
    }

    [Fact]
    public void TheZeroIsTakenAtConstructionRatherThanAtTheFirstMark()
    {
        // The span this zooms into starts where StartupTrace marked Cards, and the caller builds
        // this one statement later precisely so the two zeros agree. If the clock were read at
        // the first mark instead, everything OsdHost does before that mark — its field
        // initializers and the BandWindow base constructor among them — would vanish from a
        // table whose entire claim is that it accounts for the phase.
        long now = 500;
        var trace = new StartupWindowTrace(() => now);

        now = 530; trace.Mark(StartupWindowPhase.Fields);
        now = 540; trace.Mark(StartupWindowPhase.Weather);

        Assert.Contains("Window: 40ms total (fields 30", trace.Report(), StringComparison.Ordinal);
    }

    [Fact]
    public void APhaseNeverMarkedReadsZeroAndTheNextOneAbsorbsIt()
    {
        // A phase can legitimately not be reached: OsdHost's constructor returns early on
        // nothing today, but the marks live in two files and a future edit that stops calling one
        // must not make the columns stop adding up to the total. Reading zero is correct, and the
        // span it never used has to go somewhere rather than vanish.
        long now = 0;
        var trace = new StartupWindowTrace(() => now);

        now = 10; trace.Mark(StartupWindowPhase.Fields);
        // Shell, Band, Content and Pages never marked.
        now = 90; trace.Mark(StartupWindowPhase.Accent);
        now = 100; trace.Mark(StartupWindowPhase.Weather);

        var line = trace.Report();

        Assert.Contains("Window: 100ms total", line, StringComparison.Ordinal);
        Assert.Contains("content 0, pages 0, accent 80", line, StringComparison.Ordinal);
    }

    [Fact]
    public void AMarkOutOfOrderReadsZeroRatherThanNegative()
    {
        // Clamped rather than trusted. A negative column would be read as a phase that took no
        // time at all, which is the one reading a broken instrument must never be able to
        // produce: it looks exactly like good news.
        long now = 0;
        var trace = new StartupWindowTrace(() => now);

        now = 50; trace.Mark(StartupWindowPhase.Content);
        now = 20; trace.Mark(StartupWindowPhase.Pages);   // earlier than the mark before it
        now = 80; trace.Mark(StartupWindowPhase.Weather);

        var line = trace.Report();

        Assert.Contains("band 0, content 50, pages 0", line, StringComparison.Ordinal);
        Assert.Contains("Window: 80ms total", line, StringComparison.Ordinal);
    }

    [Fact]
    public void OnlyTheFirstMarkOfAPhaseCounts()
    {
        long now = 0;
        var trace = new StartupWindowTrace(() => now);

        now = 20; trace.Mark(StartupWindowPhase.Fields);
        now = 300; trace.Mark(StartupWindowPhase.Fields);   // ignored
        now = 40; trace.Mark(StartupWindowPhase.Weather);

        Assert.Contains("Window: 40ms total (fields 20", trace.Report(), StringComparison.Ordinal);
    }

    [Fact]
    public void ReportsOnce()
    {
        // The probe that reports this ticks four times a second for the life of the process. A
        // line per tick would bury the log it is written into.
        long now = 0;
        var trace = new StartupWindowTrace(() => now);
        now = 10; trace.Mark(StartupWindowPhase.Weather);

        Assert.NotNull(trace.Report());
        Assert.Null(trace.Report());
    }

    [Fact]
    public void AllTenPhasesAreNamedInTheLine()
    {
        // The enum and the name table are two lists that have to stay the same length, and the
        // compiler checks neither. One added member with no name beside it would throw on the
        // first launch; one name too many would report a column that never fills.
        long now = 0;
        var trace = new StartupWindowTrace(() => now);
        var line = trace.Report()!;

        foreach (var phase in Enum.GetValues<StartupWindowPhase>())
        {
            Assert.Contains(phase.ToString().ToLowerInvariant() + " ", line, StringComparison.Ordinal);
        }
    }
}
