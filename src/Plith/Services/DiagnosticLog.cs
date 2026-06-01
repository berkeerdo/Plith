using System.Globalization;
using System.IO;

namespace Plith.Services;

/// <summary>
/// Persistent file logger for boot-race + lifecycle diagnostics. Lives at
/// %LOCALAPPDATA%\Plith\plith.log. Append-only, per-write lock for thread safety
/// (NAudio callbacks fire on MTA threads).
/// </summary>
public sealed class DiagnosticLog
{
    private readonly string _logPath;
    private readonly object _lock = new();

    public string LogPath => _logPath;

    public DiagnosticLog()
    {
        var dir = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Plith");
        Directory.CreateDirectory(dir);
        _logPath = Path.Combine(dir, "plith.log");

        // Trim if log got large — keep it small (boot run only needs ~10 KB).
        try
        {
            if (File.Exists(_logPath) && new FileInfo(_logPath).Length > 256 * 1024)
                File.Delete(_logPath);
        }
        catch { /* best-effort */ }
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
            try { File.AppendAllText(_logPath, line); }
            catch { /* logging must never crash the app */ }
        }
    }
}
