using System.Globalization;
using System.IO;
using System.IO.Compression;

namespace Plith.Services;

/// <summary>
/// Packs the logs and a snapshot of the machine into one zip a person can attach to a message.
///
/// Nothing is uploaded and nothing leaves the machine on its own. The bundle is written when
/// someone clicks for it, and then it is theirs.
///
/// The environment snapshot is the part that earns this feature. A log alone answers "what did
/// Plith do"; it does not answer "what was Plith looking at". Half a day was lost to a
/// brightness feature that worked from the console and answered nothing over Remote Desktop,
/// and one line of this snapshot would have said so immediately.
/// </summary>
public static class DiagnosticBundle
{
    /// <summary>
    /// Write a zip into <paramref name="outputDirectory"/> and return its full path.
    ///
    /// The logs are copied before being zipped rather than zipped in place: the live log is
    /// open for append from other threads, and reading it directly races them.
    /// </summary>
    public static string Create(string outputDirectory, DiagnosticLog log, string environmentText, DateTime nowUtc)
    {
        Directory.CreateDirectory(outputDirectory);

        var name = string.Create(CultureInfo.InvariantCulture, $"plith-diagnostics-{nowUtc:yyyyMMdd-HHmmss}.zip");
        var zipPath = Path.Combine(outputDirectory, name);

        var staging = Path.Combine(Path.GetTempPath(), "Plith-bundle-" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(staging);

        try
        {
            File.WriteAllText(Path.Combine(staging, "environment.txt"), environmentText);
            CopyIfPresent(log.LogPath, Path.Combine(staging, "plith.log"));
            CopyIfPresent(log.PreviousLogPath, Path.Combine(staging, "plith.log.1"));

            if (File.Exists(zipPath)) File.Delete(zipPath);
            ZipFile.CreateFromDirectory(staging, zipPath);
        }
        finally
        {
            try { Directory.Delete(staging, recursive: true); } catch (IOException) { }
        }

        return zipPath;
    }

    private static void CopyIfPresent(string source, string destination)
    {
        try
        {
            if (!File.Exists(source)) return;

            // FileShare.ReadWrite because the live log is being appended to by other threads
            // while this runs. A plain File.Copy throws on that.
            using var input = new FileStream(source, FileMode.Open, FileAccess.Read, FileShare.ReadWrite);
            using var output = File.Create(destination);
            input.CopyTo(output);
        }
        catch (IOException)
        {
            // A log that cannot be read is worth less than a bundle that fails to appear.
        }
        catch (UnauthorizedAccessException)
        {
        }
    }
}
