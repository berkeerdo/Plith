using System.Diagnostics;
using System.IO;

namespace Plith.Installer.Services;

/// <summary>
/// Creates and removes the Start menu .lnk via WScript.Shell COM. The shortcut lives in
/// %ProgramData%\Microsoft\Windows\Start Menu\Programs so all users see it (matches
/// the all-users install posture). Removing the .lnk on uninstall is a single File.Delete.
///
/// Same defensive story as InstallOrchestrator.CopyWithRetry: an old .lnk left behind by
/// a previous install can carry a mangled ACL (AV quarantine, SmartScreen, whatever). The
/// WScript.Shell COM host bubbles that up as UnauthorizedAccessException with no useful
/// remediation. When it happens, we delete the stale .lnk (or reclaim ownership on it
/// first) so the fresh Save can land.
/// </summary>
public sealed class ShortcutService
{
    public static readonly string StartMenuShortcutPath = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.CommonStartMenu),
        "Programs", "Plith.lnk");

    public void CreateStartMenuShortcut(string targetExePath, string description)
    {
        try
        {
            SaveShortcut(targetExePath, description);
            return;
        }
        catch (UnauthorizedAccessException)
        {
            // Stale .lnk with a bad ACL. Reclaim ownership (or delete outright) and retry.
            RecoverStaleShortcut();
            SaveShortcut(targetExePath, description);
        }
        catch (IOException)
        {
            RecoverStaleShortcut();
            SaveShortcut(targetExePath, description);
        }
    }

    public void RemoveStartMenuShortcut()
    {
        if (!File.Exists(StartMenuShortcutPath)) return;
        try
        {
            File.Delete(StartMenuShortcutPath);
        }
        catch (UnauthorizedAccessException)
        {
            // Same recovery on uninstall — the .lnk we're trying to delete may itself
            // have a hostile ACL. takeown + icacls, then retry the delete.
            RecoverStaleShortcut();
            try { File.Delete(StartMenuShortcutPath); } catch { /* leave it; not fatal to uninstall */ }
        }
    }

    // Actual Save via WScript.Shell COM — extracted so the retry path can reuse it.
    private static void SaveShortcut(string targetExePath, string description)
    {
        var wshType = Type.GetTypeFromProgID("WScript.Shell")
            ?? throw new InvalidOperationException("WScript.Shell COM type not available.");
        dynamic shell = Activator.CreateInstance(wshType)!;

        // dynamic dispatch into WScript.Shell COM — no strongly-typed interop assembly
        // for this well-known scripting host; dynamic is the idiomatic .NET pattern here.
#pragma warning disable CA1711, CA1812
        dynamic shortcut = shell.CreateShortcut(StartMenuShortcutPath);
        shortcut.TargetPath = targetExePath;
        shortcut.WorkingDirectory = Path.GetDirectoryName(targetExePath) ?? string.Empty;
        shortcut.IconLocation = targetExePath;
        shortcut.Description = description;
        shortcut.Save();
#pragma warning restore CA1711, CA1812
    }

    private static void RecoverStaleShortcut()
    {
        // Clear any ReadOnly attribute AV may have flipped on the .lnk.
        try
        {
            if (File.Exists(StartMenuShortcutPath))
            {
                var attrs = File.GetAttributes(StartMenuShortcutPath);
                if ((attrs & FileAttributes.ReadOnly) != 0)
                    File.SetAttributes(StartMenuShortcutPath, attrs & ~FileAttributes.ReadOnly);
            }
        }
        catch { /* fall through to takeown/icacls */ }

        // takeown /F + icacls /grant *S-1-5-32-544:F — same recipe the copy path uses.
        // Best effort; ExitCode isn't checked because the subsequent delete/save will
        // fail loudly if this didn't work.
        RunElevatedTool("takeown.exe", $"/F \"{StartMenuShortcutPath}\" /A");
        RunElevatedTool("icacls.exe", $"\"{StartMenuShortcutPath}\" /grant *S-1-5-32-544:F /C /Q");

        // If the .lnk exists after ACL reset, drop it entirely so SaveShortcut writes a
        // fresh one with fresh permissions. WScript.Shell.Save doesn't reliably overwrite
        // a .lnk that ships hostile ACLs even after we've reclaimed ownership.
        try
        {
            if (File.Exists(StartMenuShortcutPath))
                File.Delete(StartMenuShortcutPath);
        }
        catch { /* let SaveShortcut throw with the real error if this fails */ }
    }

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
