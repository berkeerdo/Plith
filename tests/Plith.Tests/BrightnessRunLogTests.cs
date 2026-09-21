using Plith.Services.Brightness;

namespace Plith.Tests;

public class BrightnessRunLogTests
{
    private sealed class Sink
    {
        public List<string> Lines { get; } = new();
        public Action? Scheduled { get; private set; }
        public int ScheduleCount { get; private set; }

        public void Write(string line) => Lines.Add(line);

        public void Schedule(TimeSpan after, Action close)
        {
            Scheduled = close;
            ScheduleCount++;
        }

        public void FireClose() => Scheduled?.Invoke();
    }

    [Fact]
    public void ASinglePressWritesOneLineAndNoSummary()
    {
        var sink = new Sink();
        var log = new BrightnessRunLog(sink.Write, sink.Schedule);

        log.Step(up: true, from: 30, to: 40);
        sink.FireClose();

        Assert.Single(sink.Lines);
        Assert.Contains("30 to 40", sink.Lines[0], StringComparison.Ordinal);
    }

    [Fact]
    public void AHeldRunWritesOneLineThenOneSummary()
    {
        // This is the whole point. Fifteen writes a second, one line each, would make the log
        // useless exactly during the gesture worth diagnosing.
        var sink = new Sink();
        var log = new BrightnessRunLog(sink.Write, sink.Schedule);

        log.Step(up: true, from: 30, to: 40);
        log.Step(up: true, from: 40, to: 50);
        log.Step(up: true, from: 50, to: 60);
        sink.FireClose();

        Assert.Equal(2, sink.Lines.Count);
        Assert.Contains("3 steps", sink.Lines[1], StringComparison.Ordinal);
        Assert.Contains("30 to 60", sink.Lines[1], StringComparison.Ordinal);
    }

    [Fact]
    public void EveryStepPushesTheCloseOutSoSilenceEndsTheRun()
    {
        var sink = new Sink();
        var log = new BrightnessRunLog(sink.Write, sink.Schedule);

        log.Step(up: true, from: 30, to: 40);
        log.Step(up: true, from: 40, to: 50);

        Assert.Equal(2, sink.ScheduleCount);
    }

    [Fact]
    public void ChangingDirectionClosesTheRunAndStartsAnother()
    {
        var sink = new Sink();
        var log = new BrightnessRunLog(sink.Write, sink.Schedule);

        log.Step(up: true, from: 30, to: 40);
        log.Step(up: true, from: 40, to: 50);
        log.Step(up: false, from: 50, to: 40);
        sink.FireClose();

        // Opening line, the summary of the run that ended, then the opening line of the new one.
        Assert.Equal(3, sink.Lines.Count);
        Assert.Contains("2 steps", sink.Lines[1], StringComparison.Ordinal);
        Assert.Contains("Dimmer", sink.Lines[2], StringComparison.Ordinal);
    }

    [Fact]
    public void ARefusedWriteIsCarriedIntoTheSummary()
    {
        // Without this a monitor that refuses every write looks exactly like one that works.
        var sink = new Sink();
        var log = new BrightnessRunLog(sink.Write, sink.Schedule);

        log.Step(up: true, from: 30, to: 40);
        log.NoteRefusal("Generic PnP Monitor#0");
        log.Step(up: true, from: 40, to: 50);
        sink.FireClose();

        Assert.Contains("1 refused", sink.Lines[1], StringComparison.Ordinal);
    }

    [Fact]
    public void ARefusedWriteOnASinglePressIsStillReported()
    {
        var sink = new Sink();
        var log = new BrightnessRunLog(sink.Write, sink.Schedule);

        log.Step(up: true, from: 30, to: 40);
        log.NoteRefusal("Generic PnP Monitor#0");
        sink.FireClose();

        Assert.Equal(2, sink.Lines.Count);
        Assert.Contains("refused", sink.Lines[1], StringComparison.Ordinal);
    }

    [Fact]
    public void ARefusalWithNoRunOpenGetsItsOwnLine()
    {
        var sink = new Sink();
        var log = new BrightnessRunLog(sink.Write, sink.Schedule);

        log.NoteRefusal("Generic PnP Monitor#0");

        Assert.Single(sink.Lines);
        Assert.Contains("outside a run", sink.Lines[0], StringComparison.Ordinal);
    }

    [Fact]
    public void ClosingWithNoRunOpenWritesNothing()
    {
        var sink = new Sink();
        var log = new BrightnessRunLog(sink.Write, sink.Schedule);

        sink.FireClose();
        log.Close();

        Assert.Empty(sink.Lines);
    }

    [Fact]
    public void ARunIsClosedOnlyOnce()
    {
        var sink = new Sink();
        var log = new BrightnessRunLog(sink.Write, sink.Schedule);

        log.Step(up: true, from: 30, to: 40);
        log.Step(up: true, from: 40, to: 50);
        sink.FireClose();
        sink.FireClose();

        Assert.Equal(2, sink.Lines.Count);
    }
}
