using Plith.Services.Brightness;

namespace Plith.Tests;

public class BrightnessWriterTests
{
    private sealed class RecordingDevice : IBrightnessDevice
    {
        public string Id => "recording";
        public List<int> Writes { get; } = new();

        /// <summary>Runs inside TryWrite, so a test can make something happen while a write
        /// is in flight. That is the only moment coalescing is observable.</summary>
        public Action? DuringWrite { get; set; }

        public bool TryRead(out BrightnessReading reading)
        {
            reading = new BrightnessReading(0, 30, 100);
            return true;
        }

        public bool TryWrite(int value)
        {
            Writes.Add(value);
            DuringWrite?.Invoke();
            return true;
        }
    }

    private sealed class RefusingDevice : IBrightnessDevice
    {
        public string Id => "refusing";
        public bool TryRead(out BrightnessReading reading) { reading = default; return false; }
        public bool TryWrite(int value) => false;
    }

    /// <summary>Holds the pump instead of running it, so the test decides when work happens.
    /// A real runner hands the work to the thread pool, which would make every assertion here
    /// a race.</summary>
    private sealed class ManualRunner
    {
        private readonly Queue<Action> _pending = new();
        public void Post(Action work) => _pending.Enqueue(work);
        public int PendingCount => _pending.Count;
        public void RunAll() { while (_pending.Count > 0) _pending.Dequeue()(); }
    }

    [Fact]
    public void ARequestWritesTheValue()
    {
        var device = new RecordingDevice();
        var runner = new ManualRunner();
        var writer = new BrightnessWriter([device], runner.Post);

        writer.Request(40);
        runner.RunAll();

        Assert.Equal([40], device.Writes);
    }

    [Fact]
    public void RequestsThatArriveBeforeThePumpRunsCollapseToTheLast()
    {
        // Holding a brightness key produces far more requests than the hardware can take.
        // A write was measured at 56 ms, so a queue would keep changing the screen long
        // after the key came up.
        var device = new RecordingDevice();
        var runner = new ManualRunner();
        var writer = new BrightnessWriter([device], runner.Post);

        writer.Request(40);
        writer.Request(50);
        writer.Request(60);
        runner.RunAll();

        Assert.Equal([60], device.Writes);
    }

    [Fact]
    public void OnlyOnePumpIsEverPosted()
    {
        var runner = new ManualRunner();
        var writer = new BrightnessWriter([new RecordingDevice()], runner.Post);

        writer.Request(40);
        writer.Request(50);

        Assert.Equal(1, runner.PendingCount);
    }

    [Fact]
    public void ARequestArrivingDuringAWriteIsPickedUpByTheSamePump()
    {
        var device = new RecordingDevice();
        var runner = new ManualRunner();
        var writer = new BrightnessWriter([device], runner.Post);

        var once = false;
        device.DuringWrite = () =>
        {
            if (once) return;
            once = true;
            writer.Request(90);
        };

        writer.Request(40);
        runner.RunAll();

        // The second value is written without a second pump being posted, which is what keeps
        // a held key producing a smooth run rather than a stall at the first value.
        Assert.Equal([40, 90], device.Writes);
        Assert.Equal(0, runner.PendingCount);
    }

    [Fact]
    public void EveryDeviceGetsTheValue()
    {
        var a = new RecordingDevice();
        var b = new RecordingDevice();
        var runner = new ManualRunner();
        var writer = new BrightnessWriter([a, b], runner.Post);

        writer.Request(55);
        runner.RunAll();

        Assert.Equal([55], a.Writes);
        Assert.Equal([55], b.Writes);
    }

    [Fact]
    public void WroteCarriesTheValueThatLanded()
    {
        var runner = new ManualRunner();
        var writer = new BrightnessWriter([new RecordingDevice()], runner.Post);
        var seen = new List<int>();
        writer.Wrote += v => seen.Add(v);

        writer.Request(70);
        runner.RunAll();

        Assert.Equal([70], seen);
    }

    [Fact]
    public void ARefusedWriteIsAnnounced()
    {
        // A monitor that refuses every write looks exactly like one that works unless the
        // refusal leaves a trace. This is what the log reports.
        var runner = new ManualRunner();
        var writer = new BrightnessWriter([new RefusingDevice()], runner.Post);
        var refused = new List<string>();
        writer.Refused += id => refused.Add(id);

        writer.Request(35);
        runner.RunAll();

        Assert.Equal(["refusing"], refused);
    }

    [Fact]
    public void ADeviceThatRefusesAWriteDoesNotStopTheOthers()
    {
        var refusing = new RefusingDevice();
        var working = new RecordingDevice();
        var runner = new ManualRunner();
        var writer = new BrightnessWriter([refusing, working], runner.Post);

        writer.Request(35);
        runner.RunAll();

        Assert.Equal([35], working.Writes);
    }
}
