using System.Globalization;
using System.IO;

namespace Plith.DropCatcher;

/// <summary>
/// The catcher's own log, beside Plith's and deliberately not the same file: two processes
/// appending to one file interleave partial lines, and this one's whole job is to be readable
/// when the interesting question is what happened in the half-second the notch was hidden.
/// </summary>
internal sealed class CatcherLog
{
    private readonly string _path;
    private readonly Lock _lock = new();

    public CatcherLog()
    {
        var dir = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Plith");
        Directory.CreateDirectory(dir);
        _path = Path.Combine(dir, "dropcatcher.log");

        try
        {
            if (File.Exists(_path) && new FileInfo(_path).Length > 256 * 1024) File.Delete(_path);
        }
        catch (IOException) { /* best-effort */ }
        catch (UnauthorizedAccessException) { /* best-effort */ }
    }

    /// <summary>Named LogPath, not Path, so it does not shadow System.IO.Path inside this type.</summary>
    public string LogPath => _path;

    public void Info(string message)
    {
        var line = string.Format(CultureInfo.InvariantCulture,
            "[{0:yyyy-MM-ddTHH:mm:ss.fffZ}] {1}\r\n", DateTime.UtcNow, message);
        lock (_lock)
        {
            try { File.AppendAllText(_path, line); }
            catch (IOException) { /* logging must never crash the catcher */ }
            catch (UnauthorizedAccessException) { }
        }
    }
}
