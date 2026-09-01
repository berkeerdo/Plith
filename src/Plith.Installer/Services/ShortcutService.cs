using System.Diagnostics;
using System.IO;
using System.Runtime.InteropServices;

namespace Plith.Installer.Services;

/// <summary>
/// Creates and removes the Plith Start Menu .lnk. Preferred target is the all-users
/// Start Menu under %ProgramData%\Microsoft\Windows\Start Menu\Programs; if that path
/// refuses writes (some environments protect the tree at a level even elevated
/// installers can't reach — TrustedInstaller ownership, Windows Defender file
/// integrity monitoring, Norton Data Protector), the shortcut lands in the current
/// user's Start Menu instead. Under no circumstance does a shortcut failure block the
/// install: the app can still launch from tray auto-start and from Add/Remove
/// Programs, so we surface the failure to the log and keep going.
/// </summary>
public sealed class ShortcutService
{
    public static readonly string StartMenuShortcutPath = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.CommonStartMenu),
        "Programs", "Plith.lnk");

    private static readonly string UserStartMenuShortcutPath = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.StartMenu),
        "Programs", "Plith.lnk");

    private const uint MOVEFILE_REPLACE_EXISTING = 0x1;
    private const uint MOVEFILE_COPY_ALLOWED = 0x2;
    private const uint MOVEFILE_WRITE_THROUGH = 0x8;

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool MoveFileEx(string lpExistingFileName, string? lpNewFileName, uint dwFlags);

    private readonly LogService? _log;

    public ShortcutService() : this(null) { }

    // Ctor overload so InstallOrchestrator can wire the shared LogService in — we log
    // the takeown / icacls exit codes so failure diagnostics don't disappear silently.
    public ShortcutService(LogService? log)
    {
        _log = log;
    }

    public void CreateStartMenuShortcut(string targetExePath, string description)
    {
        // Build the .lnk in a place we can always write to (user's %TEMP%). WScript.Shell
        // saves ONLY where it's told; skipping the protected directories during the create
        // step removes an entire class of "some AV blocked shortcut creation" failures.
        var tempLnk = Path.Combine(Path.GetTempPath(), $"Plith-shortcut-{Guid.NewGuid():N}.lnk");
        try
        {
            SaveShortcutTo(tempLnk, targetExePath, description);

            // Try the all-users Start Menu first; fall back to the current user's Start
            // Menu on any failure. The user-scope path is under %APPDATA% which is
            // always writable for the running user, so if this also fails something
            // truly unusual is going on and we surrender gracefully.
            if (TryPlaceShortcut(tempLnk, StartMenuShortcutPath, scope: "all users"))
                return;
            if (TryPlaceShortcut(tempLnk, UserStartMenuShortcutPath, scope: "current user"))
                return;

            // Both targets refused. Install still succeeds — Plith launches from tray
            // auto-start and from Add/Remove Programs. Just note the miss.
            _log?.Warn($"Shortcut: could not place .lnk at either '{StartMenuShortcutPath}' or '{UserStartMenuShortcutPath}'. Install continuing without a Start Menu entry.");
        }
        finally
        {
            try { if (File.Exists(tempLnk)) File.Delete(tempLnk); } catch { /* not fatal */ }
        }
    }

    public void RemoveStartMenuShortcut()
    {
        TryDeleteShortcut(StartMenuShortcutPath);
        TryDeleteShortcut(UserStartMenuShortcutPath);
    }

    private static void SaveShortcutTo(string lnkPath, string targetExePath, string description)
    {
        var wshType = Type.GetTypeFromProgID("WScript.Shell")
            ?? throw new InvalidOperationException("WScript.Shell COM type not available.");
        dynamic shell = Activator.CreateInstance(wshType)!;

        // dynamic dispatch into WScript.Shell COM — no strongly-typed interop assembly
        // for this well-known scripting host; dynamic is the idiomatic .NET pattern here.
#pragma warning disable CA1711, CA1812
        dynamic shortcut = shell.CreateShortcut(lnkPath);
        shortcut.TargetPath = targetExePath;
        shortcut.WorkingDirectory = Path.GetDirectoryName(targetExePath) ?? string.Empty;
        shortcut.IconLocation = targetExePath;
        shortcut.Description = description;
        shortcut.Save();
#pragma warning restore CA1711, CA1812
    }

    private bool TryPlaceShortcut(string sourceLnk, string destPath, string scope)
    {
        try
        {
            var dir = Path.GetDirectoryName(destPath)!;
            Directory.CreateDirectory(dir);

            // Attempt 1: MoveFileEx atomic swap. Different kernel path than File.Copy;
            // often crosses ACL walls that trip Save-in-place.
            if (MoveFileEx(sourceLnk, destPath, MOVEFILE_REPLACE_EXISTING | MOVEFILE_COPY_ALLOWED | MOVEFILE_WRITE_THROUGH))
            {
                _log?.Info($"Shortcut: placed at {scope} '{destPath}' via MoveFileEx");
                return true;
            }

            // Attempt 2: ACL reclaim then retry. Ownership escalation both on the file
            // (if it exists) and on the parent directory.
            var reclaimResult = ReclaimOwnership(destPath);
            _log?.Info($"Shortcut: takeown/icacls on {scope} target -> {reclaimResult}");
            if (MoveFileEx(sourceLnk, destPath, MOVEFILE_REPLACE_EXISTING | MOVEFILE_COPY_ALLOWED | MOVEFILE_WRITE_THROUGH))
            {
                _log?.Info($"Shortcut: placed at {scope} '{destPath}' via MoveFileEx after reclaim");
                return true;
            }

            // Attempt 3: File.Copy overwrite — surfaces a real .NET Win32 exception if
            // it can't win either, useful for the log.
            File.Copy(sourceLnk, destPath, overwrite: true);
            _log?.Info($"Shortcut: placed at {scope} '{destPath}' via File.Copy");
            return true;
        }
        catch (Exception ex) when (ex is IOException || ex is UnauthorizedAccessException)
        {
            _log?.Warn($"Shortcut: {scope} target refused: {ex.GetType().Name}: {ex.Message}");
            return false;
        }
    }

    private void TryDeleteShortcut(string path)
    {
        if (!File.Exists(path)) return;
        try
        {
            File.Delete(path);
        }
        catch (Exception ex) when (ex is IOException || ex is UnauthorizedAccessException)
        {
            _log?.Warn($"Shortcut: delete of '{path}' failed, attempting ACL reclaim: {ex.Message}");
            ReclaimOwnership(path);
            try { File.Delete(path); }
            catch (Exception ex2) when (ex2 is IOException || ex2 is UnauthorizedAccessException)
            {
                _log?.Warn($"Shortcut: could not remove '{path}' — leaving it in place: {ex2.Message}");
            }
        }
    }

    // Runs takeown /F + icacls /grant on the file (if present) AND the parent dir.
    // Returns a short summary string ("fileOwn=OK dirOwn=OK dirAcl=OK") for the log so
    // silent failures become visible instead of vanishing behind bool discards.
    private static string ReclaimOwnership(string target)
    {
        var dir = Path.GetDirectoryName(target) ?? string.Empty;
        var parts = new List<string>();

        // Clear ReadOnly on the file first so takeown / delete downstream can bite.
        try
        {
            if (File.Exists(target))
            {
                var attrs = File.GetAttributes(target);
                if ((attrs & FileAttributes.ReadOnly) != 0)
                    File.SetAttributes(target, attrs & ~FileAttributes.ReadOnly);
            }
        }
        catch { /* fall through */ }

        if (File.Exists(target))
        {
            parts.Add($"fileOwn={ToStatus(RunElevatedTool("takeown.exe", $"/F \"{target}\" /A"))}");
            parts.Add($"fileAcl={ToStatus(RunElevatedTool("icacls.exe", $"\"{target}\" /grant *S-1-5-32-544:F /C /Q"))}");
        }
        if (!string.IsNullOrEmpty(dir))
        {
            parts.Add($"dirOwn={ToStatus(RunElevatedTool("takeown.exe", $"/F \"{dir}\" /A"))}");
            parts.Add($"dirAcl={ToStatus(RunElevatedTool("icacls.exe", $"\"{dir}\" /grant *S-1-5-32-544:F /C /Q"))}");
        }

        // Drop the stale file so the next write lands on a fresh path with fresh perms.
        try
        {
            if (File.Exists(target))
                File.Delete(target);
        }
        catch { /* the retry copy will surface the real error */ }

        return string.Join(" ", parts);
    }

    private static string ToStatus(bool ok) => ok ? "OK" : "FAIL";

    private static bool RunElevatedTool(string exe, string args)
    {
        try
        {
            var psi = new ProcessStartInfo(exe, args)
            {
                UseShellExecute = false,
                CreateNoWindow = true,
                RedirectStandardOutput = true,
                RedirectStandardError = true,
                WindowStyle = ProcessWindowStyle.Hidden,
            };
            using var proc = Process.Start(psi);
            if (proc is null) return false;
            if (!proc.WaitForExit(5000))
            {
                try { proc.Kill(); } catch { /* zombie */ }
                return false;
            }
            return proc.ExitCode == 0;
        }
        catch { return false; }
    }
}
