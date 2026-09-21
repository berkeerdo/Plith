using System.IO;
using System.IO.Compression;
using Plith.Services;

namespace Plith.Tests;

public class DiagnosticBundleTests
{
    private static string NewDir()
    {
        var dir = Path.Combine(Path.GetTempPath(), "PlithTests", Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(dir);
        return dir;
    }

    private static readonly DateTime Stamp = new(2026, 9, 18, 16, 5, 0, DateTimeKind.Utc);

    [Fact]
    public void TheBundleCarriesTheLogAndTheEnvironment()
    {
        var dir = NewDir();
        var log = new DiagnosticLog(Path.Combine(dir, "logs", "plith.log"), maxBytes: 4096);
        log.Info("Test", "a line worth shipping");

        var zip = DiagnosticBundle.Create(Path.Combine(dir, "out"), log, "ENVIRONMENT BODY", Stamp);

        using var archive = ZipFile.OpenRead(zip);
        Assert.Contains(archive.Entries, e => e.Name == "plith.log");
        Assert.Contains(archive.Entries, e => e.Name == "environment.txt");
    }

    [Fact]
    public void TheRotatedLogComesToo()
    {
        // The interesting moment is often just before a rotation, so shipping only the live
        // file would routinely ship the half without the answer in it.
        var dir = NewDir();
        var log = new DiagnosticLog(Path.Combine(dir, "logs", "plith.log"), maxBytes: 256);
        for (var i = 0; i < 40; i++) log.Info("Test", $"line {i}");

        var zip = DiagnosticBundle.Create(Path.Combine(dir, "out"), log, "ENVIRONMENT BODY", Stamp);

        using var archive = ZipFile.OpenRead(zip);
        Assert.Contains(archive.Entries, e => e.Name == "plith.log.1");
    }

    [Fact]
    public void AMissingLogDoesNotStopTheBundle()
    {
        // Someone reporting a problem on a fresh install has no log yet, and that is exactly
        // when refusing to produce a bundle would be least helpful.
        var dir = NewDir();
        var log = new DiagnosticLog(Path.Combine(dir, "logs", "plith.log"), maxBytes: 4096);

        var zip = DiagnosticBundle.Create(Path.Combine(dir, "out"), log, "ENVIRONMENT BODY", Stamp);

        Assert.True(File.Exists(zip));
        using var archive = ZipFile.OpenRead(zip);
        Assert.Contains(archive.Entries, e => e.Name == "environment.txt");
    }

    [Fact]
    public void TheNameCarriesTheTimeSoTwoBundlesDoNotCollide()
    {
        var dir = NewDir();
        var log = new DiagnosticLog(Path.Combine(dir, "logs", "plith.log"), maxBytes: 4096);

        var zip = DiagnosticBundle.Create(Path.Combine(dir, "out"), log, "body", Stamp);

        Assert.Equal("plith-diagnostics-20260918-160500.zip", Path.GetFileName(zip));
    }

    [Fact]
    public void TheLiveLogCanBeCollectedWhileItIsBeingWritten()
    {
        // The log is append-open from other threads whenever the app is running, which is
        // always when someone clicks for a bundle. A plain File.Copy throws on that.
        var dir = NewDir();
        var logPath = Path.Combine(dir, "logs", "plith.log");
        var log = new DiagnosticLog(logPath, maxBytes: 4096);
        log.Info("Test", "before");

        using var holdingItOpen = new FileStream(logPath, FileMode.Open, FileAccess.Write, FileShare.ReadWrite);

        var zip = DiagnosticBundle.Create(Path.Combine(dir, "out"), log, "body", Stamp);

        using var archive = ZipFile.OpenRead(zip);
        Assert.Contains(archive.Entries, e => e.Name == "plith.log");
    }

    [Fact]
    public void TheBundleStagingDirectoryIsCleanedUp()
    {
        var dir = NewDir();
        var log = new DiagnosticLog(Path.Combine(dir, "logs", "plith.log"), maxBytes: 4096);
        var before = Directory.GetDirectories(Path.GetTempPath(), "Plith-bundle-*").Length;

        DiagnosticBundle.Create(Path.Combine(dir, "out"), log, "body", Stamp);

        Assert.Equal(before, Directory.GetDirectories(Path.GetTempPath(), "Plith-bundle-*").Length);
    }
}
