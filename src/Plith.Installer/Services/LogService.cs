using System.Globalization;
using System.IO;

namespace Plith.Installer.Services;

/// <summary>
/// Append-only diagnostic log for the installer. Lives at
/// %LOCALAPPDATA%\Plith\Installer\install.log so the ErrorPage's "Open log" and
/// "Copy log" buttons can surface it on failure. Per-write lock; install steps
/// run on the dispatcher but exceptions can hit a background thread.
/// </summary>
public sealed class LogService
{
    private readonly string _logPath;
    private readonly object _lock = new();

    public string LogPath => _logPath;

    public LogService()
    {
        var dir = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
            "Plith", "Installer");
        Directory.CreateDirectory(dir);
        _logPath = Path.Combine(dir, "install.log");
    }

    public void Info(string message) => Write("INFO", message);

    public void Error(string step, Exception ex)
    {
        Write("ERROR", $"step={step} type={ex.GetType().Name} message={ex.Message}");
        Write("ERROR", $"stack={ex.StackTrace}");
    }

    public string ReadAll()
    {
        lock (_lock)
        {
            try { return File.ReadAllText(_logPath); }
            catch { return string.Empty; }
        }
    }

    private void Write(string level, string message)
    {
        var line = string.Format(CultureInfo.InvariantCulture,
            "[{0:yyyy-MM-ddTHH:mm:ss.fffZ}] [{1}] {2}\r\n",
            DateTime.UtcNow, level, message);
        lock (_lock)
        {
            try { File.AppendAllText(_logPath, line); }
            catch { /* logging must never crash the installer */ }
        }
    }
}
