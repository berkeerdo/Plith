using System.Diagnostics;
using System.IO;

namespace Plith.Services.Shelf;

/// <summary>
/// Starts the drop catcher, and the route it takes is the whole content of this class.
///
/// Plith runs at High integrity because it has UIAccess, and a child process inherits its
/// parent's token. Started the obvious way — Process.Start on the exe — the catcher would come up
/// at HIGH and be exactly as unable to receive a drop as the window it exists to stand in for,
/// while looking perfectly healthy: it starts, it connects, it shows itself, and no drop ever
/// arrives. Handing the path to the already-running Explorer instead makes Explorer the parent,
/// so the catcher gets Explorer's MEDIUM token.
///
/// The catcher logs its own integrity level on every start for the same reason. This failure
/// mode is indistinguishable from the bug the catcher was built to fix, so it is named out loud
/// rather than inferred later from an absence.
/// </summary>
public static class DropCatcherLauncher
{
    public const string ProcessName = "Plith.DropCatcher";
    public const string ExecutableName = ProcessName + ".exe";

    /// <summary>Beside Plith.exe. Both the installed layout and the build output put them in the
    /// same directory, so there is no configuration for this and deliberately so.</summary>
    public static string ResolveExecutablePath(string plithDirectory)
        => Path.Combine(plithDirectory, ExecutableName);

    /// <summary>
    /// Returns false when nothing was started — either one is already running, or the exe is not
    /// beside Plith. Neither is an error worth a dialog: the shelf simply does not work, and the
    /// log says which of the two it was.
    /// </summary>
    public static bool EnsureRunning(DiagnosticLog? log = null)
    {
        if (Process.GetProcessesByName(ProcessName).Length > 0)
        {
            log?.Info("Shelf", "Drop catcher is already running.");
            return false;
        }

        var directory = Path.GetDirectoryName(Environment.ProcessPath);
        if (directory is null)
        {
            log?.Warn("Shelf", "Cannot locate Plith's own directory; drop catcher not started.");
            return false;
        }

        var exe = ResolveExecutablePath(directory);
        if (!File.Exists(exe))
        {
            log?.Warn("Shelf", $"Drop catcher not found at {exe}; the shelf will not receive drops.");
            return false;
        }

        try
        {
            // UseShellExecute so this is a shell verb rather than a direct CreateProcess: the
            // running Explorer picks the path up and launches it with its own token.
            //
            // Measured, both ways, from an elevated parent: launched directly the catcher comes
            // up HIGH; handed over like this it comes up MEDIUM.
            //
            // Nothing may be appended after the path. Explorer treats its command line as a
            // thing to open and does not forward arguments to the target, so an argument added
            // here would simply never arrive — silently, with the catcher starting normally
            // without it. The catcher takes no arguments in this mode for exactly that reason;
            // its --probe mode is a hand-run tool and is not reachable through here.
            using var started = Process.Start(new ProcessStartInfo("explorer.exe", $"\"{exe}\"")
            {
                UseShellExecute = true,
                WindowStyle = ProcessWindowStyle.Hidden,
            });

            log?.Info("Shelf", $"Asked Explorer to start the drop catcher: {exe}");
            return true;
        }
        catch (System.ComponentModel.Win32Exception ex)
        {
            log?.Warn("Shelf", $"Could not start the drop catcher: {ExceptionText.Describe(ex)}");
            return false;
        }
    }
}
