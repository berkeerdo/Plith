using Plith.Services.Brightness;

namespace Plith.Tests;

public class BrightnessLevelCacheTests
{
    [Fact]
    public void AnEmptyCacheKnowsNothing()
    {
        Assert.False(new BrightnessLevelCache().TryGet(out _));
    }

    [Fact]
    public void AStoredReadingComesBack()
    {
        var cache = new BrightnessLevelCache();
        cache.Set(new BrightnessReading(0, 30, 100));

        Assert.True(cache.TryGet(out var reading));
        Assert.Equal(new BrightnessReading(0, 30, 100), reading);
    }

    [Fact]
    public void AWrittenValueMovesTheLevelWithoutTouchingTheRange()
    {
        // This is what saves the round trip: after a write we know where the level is, so the
        // next press in the same run does not have to ask the monitor again. A read costs
        // 60 ms on the hardware this was measured on, the same as a write.
        var cache = new BrightnessLevelCache();
        cache.Set(new BrightnessReading(20, 30, 80));

        cache.NoteWritten(50);

        Assert.True(cache.TryGet(out var reading));
        Assert.Equal(new BrightnessReading(20, 50, 80), reading);
    }

    [Fact]
    public void AWrittenValueWithNothingCachedIsIgnored()
    {
        // The range came from a real read. Building a reading out of a written value alone
        // would be inventing the minimum and the maximum.
        var cache = new BrightnessLevelCache();

        cache.NoteWritten(50);

        Assert.False(cache.TryGet(out _));
    }

    [Fact]
    public void InvalidatingForgetsEverything()
    {
        // Someone can change brightness from the monitor's own buttons and nothing tells us,
        // so the cache is dropped when a run of presses ends and every gesture starts from a
        // real reading.
        var cache = new BrightnessLevelCache();
        cache.Set(new BrightnessReading(0, 30, 100));

        cache.Invalidate();

        Assert.False(cache.TryGet(out _));
    }
}
