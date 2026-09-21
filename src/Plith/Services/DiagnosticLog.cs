using System.Globalization;
using System.IO;
using System.Text;

namespace Plith.Services;

/// <summary>
/// Persistent file logger for lifecycle and diagnostic traces. Lives at
/// %LOCALAPPDATA%\Plith\plith.log. Append-only, written on every call rather than buffered,
/// because most of what this file is for is the moment before something went wrong.
///
/// Bounded by rotation rather than by deletion. The earlier version checked the size only at
/// startup and deleted the whole file when it was over: unbounded during a session, and then
/// the entire history gone at once. Now the live file rotates to a single ".1" at the cap, so
/// the worst case on disk is two files and the previous run's tail always survives.
///
/// Per-write lock for thread safety: NAudio callbacks and WMI events both arrive on threadpool
/// threads.
/// </summary>
public sealed class DiagnosticLog
{
    /// <summary>Per file, so the pair is bounded at twice this. Small enough that a person can
    /// mail the bundle, large enough to hold a long session.</summary>
    public const long DefaultMaxBytes = 512 * 1024;

    private readonly string _logPath;
    private readonly long _maxBytes;
    private readonly object _lock = new();
    private long _length;

    public string LogPath => _logPath;

    /// <summary>Where the previous span of the log went. Collected by the diagnostic bundle,
    /// because the interesting event is often just before the rotation.</summary>
    public string PreviousLogPath => _logPath + ".1";

    public DiagnosticLog() : this(DefaultPath(), DefaultMaxBytes) { }

    /// <summary>Test-friendly ctor: caller supplies the path and the cap.</summary>
    public DiagnosticLog(string logPath, long maxBytes)
    {
        _logPath = logPath;
        _maxBytes = maxBytes;

        try
        {
            var dir = Path.GetDirectoryName(_logPath);
            if (dir is not null) Directory.CreateDirectory(dir);
            _length = File.Exists(_logPath) ? new FileInfo(_logPath).Length : 0;
        }
        catch (IOException) { _length = 0; }
        catch (UnauthorizedAccessException) { _length = 0; }
    }

    private static string DefaultPath()
    {
        var dir = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Plith");
        return Path.Combine(dir, "plith.log");
    }

    public void Info(string source, string message) => Write("INFO", source, message);

    public void Warn(string source, string message) => Write("WARN", source, message);

    public void Error(string source, string message) => Write("ERROR", source, message);

    private void Write(string level, string source, string message)
    {
        var line = string.Format(CultureInfo.InvariantCulture,
            "[{0:yyyy-MM-ddTHH:mm:ss.fffZ}] [{1}] [{2}] {3}\r\n",
            DateTime.UtcNow, level, source, message);

        lock (_lock)
        {
            try
            {
                File.AppendAllText(_logPath, line);

                // Counted rather than measured. A FileInfo per line would stat the file on
                // every write, on a path that is called from audio and WMI callbacks.
                _length += Encoding.UTF8.GetByteCount(line);
                if (_length > _maxBytes) Rotate();
            }
            catch
            {
                // Logging must never crash the app.
            }
        }
    }

    /// <summary>
    /// Move the live file aside and start a new one.
    ///
    /// A rename rather than trimming the head of the file: trimming means rewriting the whole
    /// thing, which is both slower and racy against a reader. The cost is that the boundary
    /// falls wherever the cap lands rather than on a tidy line count, which nobody reading a
    /// log cares about.
    /// </summary>
    private void Rotate()
    {
        try
        {
            if (File.Exists(PreviousLogPath)) File.Delete(PreviousLogPath);
            File.Move(_logPath, PreviousLogPath);
        }
        catch (IOException) { return; }
        catch (UnauthorizedAccessException) { return; }

        _length = 0;
    }
}
