using Plith.Services.Brightness;

namespace Plith.Tests;

public class BrightnessDiscoveryTests
{
    /// <summary>A device whose behaviour the test controls. It models the monitor this
    /// feature was measured on, which reports no capabilities at all while reading and
    /// writing perfectly.</summary>
    private sealed class FakeDevice(string id, bool readSucceeds) : IBrightnessDevice
    {
        public string Id => id;
        public int Reads { get; private set; }

        public bool TryRead(out BrightnessReading reading)
        {
            Reads++;
            reading = readSucceeds ? new BrightnessReading(0, 30, 100) : default;
            return readSucceeds;
        }

        public bool TryWrite(int value) => readSucceeds;
    }

    [Fact]
    public void ADeviceThatAnswersAReadIsKept()
    {
        var kept = BrightnessDiscovery.KeepAnswering([new FakeDevice("a", readSucceeds: true)]);
        Assert.Single(kept);
        Assert.Equal("a", kept[0].Id);
    }

    [Fact]
    public void ADeviceThatCannotBeReadIsDropped()
    {
        var kept = BrightnessDiscovery.KeepAnswering([new FakeDevice("a", readSucceeds: false)]);
        Assert.Empty(kept);
    }

    [Fact]
    public void EachCandidateIsAskedExactlyOnce()
    {
        // A read costs a DDC/CI round trip, measured at 56 ms on the monitor this was built
        // against. Discovery asking twice would double a cost that is already visible.
        var device = new FakeDevice("a", readSucceeds: true);
        BrightnessDiscovery.KeepAnswering([device]);
        Assert.Equal(1, device.Reads);
    }

    [Fact]
    public void TheAnsweringOnesSurviveAlongsideTheSilentOnes()
    {
        var kept = BrightnessDiscovery.KeepAnswering(
        [
            new FakeDevice("silent", readSucceeds: false),
            new FakeDevice("answers", readSucceeds: true),
        ]);

        Assert.Single(kept);
        Assert.Equal("answers", kept[0].Id);
    }
}
