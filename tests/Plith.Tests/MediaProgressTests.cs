using Plith.Services;

namespace Plith.Tests;

public class MediaProgressTests
{
    private static readonly DateTimeOffset Now = new(2026, 9, 20, 18, 0, 0, TimeSpan.Zero);

    private static TimeSpan Elapsed(double positionSeconds, double stampAgeSeconds,
                                    double durationSeconds, bool isPlaying = true)
        => MediaProgress.Elapsed(
            TimeSpan.FromSeconds(positionSeconds),
            Now - TimeSpan.FromSeconds(stampAgeSeconds),
            TimeSpan.FromSeconds(durationSeconds),
            isPlaying,
            Now);

    [Fact]
    public void Playing_AdvancesTheReportedPositionByTheAgeOfTheReading()
    {
        Assert.Equal(TimeSpan.FromSeconds(65), Elapsed(60, 5, 200));
    }

    [Fact]
    public void Paused_DoesNotDrift()
    {
        // The whole point of interpolating from a stamp: a paused track whose reading is a
        // minute old is still exactly where it was left.
        Assert.Equal(TimeSpan.FromSeconds(60), Elapsed(60, 60, 200, isPlaying: false));
    }

    [Fact]
    public void PastTheEnd_ClampsToTheDuration()
    {
        Assert.Equal(TimeSpan.FromSeconds(200), Elapsed(195, 30, 200));
    }

    [Fact]
    public void AStampInTheFuture_ReturnsTheReportedPosition()
    {
        // Clock skew, and a source that stamps with its own clock. Subtracting a negative age
        // would run the bar backwards.
        Assert.Equal(TimeSpan.FromSeconds(60), Elapsed(60, -10, 200));
    }

    [Fact]
    public void NoDuration_IsZero()
    {
        // A live stream reports no end time. The caller draws nothing in this case; this is
        // here so the function is still total.
        Assert.Equal(TimeSpan.Zero, Elapsed(60, 5, 0));
    }

    [Fact]
    public void ANegativeReportedPosition_ClampsToZero()
    {
        Assert.Equal(TimeSpan.Zero, Elapsed(-30, 0, 200));
    }

    [Fact]
    public void Clock_WritesMinutesAndPaddedSeconds()
    {
        Assert.Equal("2:27", MediaProgress.Clock(TimeSpan.FromSeconds(147)));
        Assert.Equal("0:00", MediaProgress.Clock(TimeSpan.Zero));
        Assert.Equal("0:07", MediaProgress.Clock(TimeSpan.FromSeconds(7)));
    }

    [Fact]
    public void Clock_WritesHoursOnlyWhenThereAreSome()
    {
        Assert.Equal("1:02:03", MediaProgress.Clock(TimeSpan.FromSeconds(3723)));
        Assert.Equal("59:59", MediaProgress.Clock(TimeSpan.FromSeconds(3599)));
    }

    [Fact]
    public void Clock_OfANegativeSpanIsZero()
    {
        // Remaining time is computed as duration minus elapsed, and a source that reports a
        // position past its own duration would otherwise render "-0:-5".
        Assert.Equal("0:00", MediaProgress.Clock(TimeSpan.FromSeconds(-5)));
    }

    // --- PositionFor: a fraction of the track back into a position -----------------------------

    [Fact]
    public void PositionFor_MapsAFractionOntoTheDuration()
    {
        Assert.Equal(TimeSpan.FromSeconds(100),
                     MediaProgress.PositionFor(0.5, TimeSpan.FromSeconds(200)));
    }

    [Fact]
    public void PositionFor_ClampsBothEnds()
    {
        var duration = TimeSpan.FromSeconds(200);

        Assert.Equal(TimeSpan.Zero, MediaProgress.PositionFor(-0.2, duration));
        Assert.Equal(duration, MediaProgress.PositionFor(1.4, duration));
    }

    [Fact]
    public void PositionFor_WithNoDurationIsZero()
    {
        // The bar is not drawn without a duration, so this cannot be reached through the UI. It
        // is defined anyway, because a seek computed from a zero-length track is the kind of
        // thing that arrives from a source that changed underneath the gesture.
        Assert.Equal(TimeSpan.Zero, MediaProgress.PositionFor(0.5, TimeSpan.Zero));
    }

    [Fact]
    public void PositionFor_OfSomethingThatIsNotANumberIsZero()
    {
        // A Slider whose Maximum is momentarily zero divides to NaN, and NaN passed to
        // TimeSpan.FromTicks throws. Seeking to the start is wrong in a way a person can see and
        // undo; a crash in a media page is not.
        Assert.Equal(TimeSpan.Zero, MediaProgress.PositionFor(double.NaN, TimeSpan.FromSeconds(200)));
    }

    [Fact]
    public void PositionFor_RoundsToWholeSeconds()
    {
        // Whole seconds because that is the resolution the two clocks beside the bar show. A
        // seek to 100,4 s reads back as 1:40 either way, and a rounded write is one a person can
        // repeat exactly.
        Assert.Equal(TimeSpan.FromSeconds(100),
                     MediaProgress.PositionFor(0.5012, TimeSpan.FromSeconds(200)));
    }
}
