using System.IO;
using Plith.Services;

namespace Plith.Tests;

public class DiagnosticLogTests
{
    private static string NewLogPath()
        => Path.Combine(Path.GetTempPath(), "PlithTests", Guid.NewGuid().ToString("N"), "plith.log");

    [Fact]
    public void LinesAreWrittenImmediately()
    {
        // A log that buffers is worthless for a crash, which is most of what this file is for.
        var path = NewLogPath();
        var log = new DiagnosticLog(path, maxBytes: 4096);

        log.Info("Test", "first");

        Assert.Contains("first", File.ReadAllText(path), StringComparison.Ordinal);
    }

    [Fact]
    public void PassingTheCapRotatesInsteadOfDeleting()
    {
        // The old behaviour deleted the whole file, and only at startup: unbounded during a
        // session, then the entire history gone at once.
        var path = NewLogPath();
        var log = new DiagnosticLog(path, maxBytes: 512);

        for (var i = 0; i < 40; i++) log.Info("Test", $"line {i}");

        Assert.True(File.Exists(path));
        Assert.True(File.Exists(path + ".1"));

        // The newest line is in the live file and the rotated file holds older content, which
        // is the whole contract. WHICH older line it holds is not asserted: past the cap the
        // oldest lines are gone by design, and a test that pinned a particular line would be
        // pinning the byte size of a timestamp.
        var live = File.ReadAllText(path);
        var rotated = File.ReadAllText(path + ".1");

        Assert.Contains("line 39", live, StringComparison.Ordinal);
        Assert.DoesNotContain("line 39", rotated, StringComparison.Ordinal);
        Assert.NotEmpty(rotated);
    }

    [Fact]
    public void OnlyEverTwoFiles()
    {
        // The whole point of the cap is a bound on disk. A rotation that accumulated .2, .3
        // would defeat it quietly.
        var path = NewLogPath();
        var log = new DiagnosticLog(path, maxBytes: 256);

        for (var i = 0; i < 200; i++) log.Info("Test", $"line {i}");

        var dir = Path.GetDirectoryName(path)!;
        Assert.Equal(2, Directory.GetFiles(dir).Length);
    }

    [Fact]
    public void NeitherFileGrowsPastTheCapByMuch()
    {
        var path = NewLogPath();
        const long cap = 1024;
        var log = new DiagnosticLog(path, maxBytes: cap);

        for (var i = 0; i < 200; i++) log.Info("Test", $"line {i}");

        // One line may cross the boundary before the rotation runs, so the bound is the cap
        // plus a line rather than the cap exactly.
        Assert.True(new FileInfo(path).Length <= cap + 512);
        Assert.True(new FileInfo(path + ".1").Length <= cap + 512);
    }

    [Fact]
    public void AnExistingFileIsContinuedRatherThanTruncated()
    {
        // A restart must not throw away what the previous run recorded, which is exactly the
        // part someone reporting a problem needs.
        var path = NewLogPath();
        Directory.CreateDirectory(Path.GetDirectoryName(path)!);
        File.WriteAllText(path, "earlier run\r\n");

        new DiagnosticLog(path, maxBytes: 4096).Info("Test", "later run");

        var text = File.ReadAllText(path);
        Assert.Contains("earlier run", text, StringComparison.Ordinal);
        Assert.Contains("later run", text, StringComparison.Ordinal);
    }

    [Fact]
    public void TheRotatedPathIsExposedSoABundleCanCollectIt()
    {
        var path = NewLogPath();
        var log = new DiagnosticLog(path, maxBytes: 4096);
        Assert.Equal(path, log.LogPath);
        Assert.Equal(path + ".1", log.PreviousLogPath);
    }
}
