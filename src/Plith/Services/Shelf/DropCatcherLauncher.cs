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
    /// The catcher's process id, or null when none is running.
    ///
    /// Exists for one caller: <see cref="ShelfSession"/> has to name the catcher to
    /// AllowSetForegroundWindow, and the answer has to be found the same way EnsureRunning finds
    /// it or the two could disagree about which process is the catcher. The first match, because
    /// the catcher takes a per-user mutex and a second instance exits immediately; a stale second
    /// entry here would only ever be a process on its way out.
    /// </summary>
    public static int? FindProcessId()
    {
        var expected = ExpectedExecutablePath();
        var processes = Process.GetProcessesByName(ProcessName);
        try
        {
            foreach (var process in processes)
            {
                // The name alone is not enough, and this is the one function whose whole job is
                // naming the right process. Any process on the machine may call itself
                // Plith.DropCatcher; one that did would collect the foreground grant meant for
                // the real catcher, and would also make EnsureRunning decide a catcher is already
                // up and never start one. The pipe's ACL makes the impact small; the check costs
                // a line.
                if (expected is not null && !PathMatches(process, expected)) continue;
                return process.Id;
            }

            return null;
        }
        finally
        {
            foreach (var p in processes) p.Dispose();
        }
    }

    /// <summary>Null when Plith cannot locate its own directory, which is the one case where the
    /// name is all there is to go on. Falling back to the name is deliberate: refusing to find a
    /// catcher at all would be a worse answer than finding one by name.</summary>
    private static string? ExpectedExecutablePath()
    {
        var directory = Path.GetDirectoryName(Environment.ProcessPath);
        return directory is null ? null : ResolveExecutablePath(directory);
    }

    private static bool PathMatches(Process process, string expected)
    {
        try
        {
            return string.Equals(process.MainModule?.FileName, expected, StringComparison.OrdinalIgnoreCase);
        }
        catch (System.ComponentModel.Win32Exception)
        {
            // A process this one cannot open. Not the catcher then: Plith runs at High integrity
            // and the catcher at Medium, so the real one is always readable from here.
            return false;
        }
        catch (InvalidOperationException)
        {
            // It exited between the enumeration and this call.
            return false;
        }
    }

    /// <summary>
    /// Start the catcher if it is not already up, and say which of the four things happened.
    ///
    /// It used to return a bool, which collapsed "one is already running" and "it is not
    /// installed" into the same answer. That was enough while the only consumer was the log,
    /// and stopped being enough the moment the shelf had to tell a person why a click did
    /// nothing: those two need opposite sentences, and a click that says nothing at all is
    /// indistinguishable from the product being broken.
    /// </summary>
    public static CatcherStart EnsureRunning(DiagnosticLog? log = null)
    {
        // Disposed, as FindProcessId already does with its own array: every Process handed back
        // by GetProcessesByName holds a native handle, and the shelf opens this often enough for
        // "one handle per open" to be a leak rather than a tidiness point.
        var running = Process.GetProcessesByName(ProcessName);
        try
        {
            if (running.Length > 0)
            {
                log?.Info("Shelf", "Drop catcher is already running.");
                return CatcherStart.AlreadyRunning;
            }
        }
        finally
        {
            foreach (var process in running) process.Dispose();
        }

        var directory = Path.GetDirectoryName(Environment.ProcessPath);
        if (directory is null)
        {
            log?.Warn("Shelf", "Cannot locate Plith's own directory; drop catcher not started.");
            return CatcherStart.NotFound;
        }

        var exe = ResolveExecutablePath(directory);
        if (!File.Exists(exe))
        {
            log?.Warn("Shelf", $"Drop catcher not found at {exe}; the shelf will not receive drops.");
            return CatcherStart.NotFound;
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
            return CatcherStart.Started;
        }
        catch (System.ComponentModel.Win32Exception ex)
        {
            log?.Warn("Shelf", $"Could not start the drop catcher: {ExceptionText.Describe(ex)}");
            return CatcherStart.Failed;
        }
    }
}

/// <summary>
/// Why a start attempt ended the way it did.
///
/// A bool could not tell "one is already running" from "it is not installed", and the shelf has
/// to say something different for each: one means wait a moment, the other means this install is
/// missing a file and waiting will never help.
///
/// <see cref="Started"/> is NOT the same as ready. Explorer was asked to launch the catcher and
/// answered without throwing; the catcher then has to come up and connect to the pipe, which
/// takes long enough that the click which triggered the start will not be the click that gets a
/// shelf.
/// </summary>
public enum CatcherStart
{
    /// <summary>Explorer was asked to launch it, and it will connect when it is ready.</summary>
    Started,

    /// <summary>A catcher process was already running before this call.</summary>
    AlreadyRunning,

    /// <summary>The executable is not beside Plith, or Plith cannot find its own directory.</summary>
    NotFound,

    /// <summary>The launch was attempted and Windows refused it.</summary>
    Failed,
}
