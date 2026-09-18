using Plith.Services.Brightness;

namespace Plith.Tests;

public class BrightnessStepTests
{
    [Fact]
    public void AStepUpMovesByThePercentageOfTheSpan()
    {
        var r = new BrightnessReading(Min: 0, Current: 30, Max: 100);
        Assert.Equal(40, BrightnessStep.Next(r, stepPercent: 10, up: true));
    }

    [Fact]
    public void AStepDownMovesTheOtherWay()
    {
        var r = new BrightnessReading(Min: 0, Current: 30, Max: 100);
        Assert.Equal(20, BrightnessStep.Next(r, stepPercent: 10, up: false));
    }

    [Fact]
    public void TheSpanIsTheDevicesOwn_NotAssumedToBeZeroToHundred()
    {
        // DDC/CI does not require a minimum of zero. The monitor measured for this feature
        // happens to report 0 and 100, which is exactly the coincidence that hides a bug on
        // somebody else's hardware.
        var r = new BrightnessReading(Min: 20, Current: 20, Max: 80);
        Assert.Equal(26, BrightnessStep.Next(r, stepPercent: 10, up: true));
    }

    [Fact]
    public void AStepCannotLeaveTheDevicesRange()
    {
        var top = new BrightnessReading(Min: 0, Current: 97, Max: 100);
        Assert.Equal(100, BrightnessStep.Next(top, stepPercent: 10, up: true));

        var bottom = new BrightnessReading(Min: 10, Current: 12, Max: 100);
        Assert.Equal(10, BrightnessStep.Next(bottom, stepPercent: 10, up: false));
    }

    [Fact]
    public void AStepThatRoundsToNothingStillMoves()
    {
        // A small span with a small percentage rounds to zero, and a key that changes nothing
        // reads as a broken key rather than as a limit.
        var r = new BrightnessReading(Min: 0, Current: 5, Max: 10);
        Assert.Equal(6, BrightnessStep.Next(r, stepPercent: 1, up: true));
    }

    [Fact]
    public void ADeviceWithNoSpanIsLeftAlone()
    {
        var r = new BrightnessReading(Min: 50, Current: 50, Max: 50);
        Assert.Equal(50, BrightnessStep.Next(r, stepPercent: 10, up: true));
    }
}
