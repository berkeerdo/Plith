using Plith.Services;

namespace Plith.Tests;

public class VolumeMathTests
{
    [Fact]
    public void NormalizedZeroIsVoicemeetersFloor()
    {
        Assert.Equal(VolumeMath.VoicemeeterMinDb, VolumeMath.NormalizedToVoicemeeterDb(0));
    }

    [Fact]
    public void NormalizedOneIsVoicemeetersCeiling()
    {
        Assert.Equal(VolumeMath.VoicemeeterMaxDb, VolumeMath.NormalizedToVoicemeeterDb(1));
    }

    [Fact]
    public void HalfWayIsHalfWayInDb()
    {
        // Linear in dB, matching how the bar is drawn and how Voicemeeter's own faders behave.
        var expected = (VolumeMath.VoicemeeterMinDb + VolumeMath.VoicemeeterMaxDb) / 2;
        Assert.Equal(expected, VolumeMath.NormalizedToVoicemeeterDb(0.5), 4);
    }

    [Theory]
    [InlineData(-2.0)]
    [InlineData(1.5)]
    public void OutOfRangePositionsClamp(double normalized)
    {
        var db = VolumeMath.NormalizedToVoicemeeterDb(normalized);
        Assert.InRange(db, VolumeMath.VoicemeeterMinDb, VolumeMath.VoicemeeterMaxDb);
    }

    [Theory]
    [InlineData(0.0)]
    [InlineData(0.25)]
    [InlineData(0.5)]
    [InlineData(0.75)]
    [InlineData(1.0)]
    public void VoicemeeterRoundTripsThroughDb(double normalized)
    {
        var db = VolumeMath.NormalizedToVoicemeeterDb(normalized);

        Assert.Equal(normalized, VolumeMath.VoicemeeterDbToNormalized(db), 5);
    }

    [Fact]
    public void AGainSetOutsideVoicemeetersRangeClampsRatherThanDrawingPastTheEnds()
    {
        // Voicemeeter's own UI can put a bus outside the range Plith draws. A bar pinned at its
        // end is honest; one drawn past it is not drawable at all.
        Assert.Equal(0, VolumeMath.VoicemeeterDbToNormalized(-90f));
        Assert.Equal(1, VolumeMath.VoicemeeterDbToNormalized(40f));
    }

    [Fact]
    public void WindowsScalarIsTheNormalisedValueItself()
    {
        // Identity on purpose: MasterVolumeLevelScalar is already where Windows' own slider
        // sits, so any curve here would put Plith's slider somewhere else for the same level.
        Assert.Equal(0.42f, VolumeMath.NormalizedToWindowsScalar(0.42), 5);
    }

    [Fact]
    public void WindowsScalarClamps()
    {
        Assert.Equal(0f, VolumeMath.NormalizedToWindowsScalar(-0.3));
        Assert.Equal(1f, VolumeMath.NormalizedToWindowsScalar(2));
    }

    [Theory]
    [InlineData(0.47, 5, 0.45)]
    [InlineData(0.48, 5, 0.50)]
    [InlineData(0.0, 5, 0.0)]
    [InlineData(1.0, 5, 1.0)]
    public void SnapsToTheStep(double input, double step, double expected)
    {
        Assert.Equal(expected, VolumeMath.SnapToStep(input, step), 5);
    }

    [Fact]
    public void AZeroStepMeansNoSnapping()
    {
        // Asking for no snapping must give the raw position, not a division by zero.
        Assert.Equal(0.4731, VolumeMath.SnapToStep(0.4731, 0), 5);
        Assert.Equal(0.4731, VolumeMath.SnapToStep(0.4731, -1), 5);
    }

    [Fact]
    public void SnappingNeverLeavesTheRange()
    {
        Assert.Equal(1.0, VolumeMath.SnapToStep(0.99, 5), 5);
        Assert.Equal(0.0, VolumeMath.SnapToStep(0.001, 5), 5);
    }

    [Fact]
    public void PositionOnTrackIsTheFraction()
    {
        Assert.Equal(0.25, VolumeMath.PositionOnTrack(50, 200), 5);
    }

    [Fact]
    public void PositionOnTrackClampsOutsideTheTrack()
    {
        // A drag continues past the track's ends; the pointer leaving does not mean the value
        // should run away with it.
        Assert.Equal(0, VolumeMath.PositionOnTrack(-40, 200));
        Assert.Equal(1, VolumeMath.PositionOnTrack(900, 200));
    }

    [Fact]
    public void AnUnmeasuredTrackReadsAsZeroRatherThanDividing()
    {
        // A drag can start on the frame's first arrange pass, before the track has a width.
        Assert.Equal(0, VolumeMath.PositionOnTrack(50, 0));
        Assert.Equal(0, VolumeMath.PositionOnTrack(50, -10));
    }
}
