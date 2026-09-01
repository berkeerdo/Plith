using System.Diagnostics;
using System.IO;
using System.Runtime.InteropServices;

namespace Plith.Installer.Services;

/// <summary>
/// Creates and removes the Start menu .lnk via WScript.Shell COM. The shortcut lives in
/// %ProgramData%\Microsoft\Windows\Start Menu\Programs so all users see it (matches
/// the all-users install posture). Removing the .lnk on uninstall is a single File.Delete.
///
/// Two ACL escape hatches:
///   1. Save via a %TEMP% path first (always writable), then move to Start Menu. That
///      way WScript.Shell.Save never has to hit a directory whose ACL might be hostile —
///      Norton has been observed protecting ProgramData\Microsoft\ subtrees on some SKUs
///      even for elevated processes.
///   2. On the move, use MoveFileEx REPLACE_EXISTING and, if that fails, run
///      takeown + icacls on both the .lnk AND its parent directory before retrying.
/// </summary>
public sealed class ShortcutService
{
    public static readonly string StartMenuShortcutPath = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.CommonStartMenu),
        "Programs", "Plith.lnk");

    private const uint MOVEFILE_REPLACE_EXISTING = 0x1;
    private const uint MOVEFILE_COPY_ALLOWED = 0x2;
    private const uint MOVEFILE_WRITE_THROUGH = 0x8;

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool MoveFileEx(string lpExistingFileName, string? lpNewFileName, uint dwFlags);

    public void CreateStartMenuShortcut(string targetExePath, string description)
    {
        // Build the .lnk in a place we can always write to (user's %TEMP%). WScript.Shell
        // saves ONLY where it's told; skipping ProgramData for the save step removes an
        // entire class of "some AV blocked shortcut creation" failures.
        var tempLnk = Path.Combine(Path.GetTempPath(), $"Plith-shortcut-{Guid.NewGuid():N}.lnk");
        try
        {
            SaveShortcutTo(tempLnk, targetExePath, description);
            PlaceShortcutAtStartMenu(tempLnk);
        }
        finally
        {
            try { if (File.Exists(tempLnk)) File.Delete(tempLnk); } catch { /* not fatal */ }
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
            ReclaimShortcutOwnership();
            try { File.Delete(StartMenuShortcutPath); } catch { /* leave it; not fatal to uninstall */ }
        }
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

    private static void PlaceShortcutAtStartMenu(string sourceLnk)
    {
        var startMenuDir = Path.GetDirectoryName(StartMenuShortcutPath)!;
        Directory.CreateDirectory(startMenuDir);

        // Path 1: MoveFileEx REPLACE_EXISTING — atomic replace, different kernel path
        // than File.Copy(overwrite: true). Often succeeds where File.Copy hits ACL walls.
        if (MoveFileEx(sourceLnk, StartMenuShortcutPath,
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_COPY_ALLOWED | MOVEFILE_WRITE_THROUGH))
            return;

        // Path 2: Ownership escalation on BOTH the .lnk file and its parent directory
        // (some ACL breakage only shows up when the DIRECTORY refuses to accept a new
        // file — recovering just the file itself is a no-op when the file doesn't yet
        // exist). Retry the atomic replace after.
        ReclaimShortcutOwnership();
        if (MoveFileEx(sourceLnk, StartMenuShortcutPath,
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_COPY_ALLOWED | MOVEFILE_WRITE_THROUGH))
            return;

        // Path 3: Fall back to File.Copy with the same ownership reclaim already done.
        // If this fails too, the .NET exception at least surfaces a real Win32 code
        // rather than the opaque COM HRESULT WScript.Shell would have raised.
        File.Copy(sourceLnk, StartMenuShortcutPath, overwrite: true);
    }

    private static void ReclaimShortcutOwnership()
    {
        var startMenuDir = Path.GetDirectoryName(StartMenuShortcutPath)!;

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

        // takeown /F on both the file (if present) AND the parent dir so a hostile
        // ACL on either shape gets normalised. *S-1-5-32-544 is the well-known
        // Administrators SID — using the SID avoids the locale trap (BUILTIN\Administrators
        // on en-US vs BUILTIN\Yöneticiler on tr-TR).
        if (File.Exists(StartMenuShortcutPath))
        {
            RunElevatedTool("takeown.exe", $"/F \"{StartMenuShortcutPath}\" /A");
            RunElevatedTool("icacls.exe", $"\"{StartMenuShortcutPath}\" /grant *S-1-5-32-544:F /C /Q");
        }
        RunElevatedTool("takeown.exe", $"/F \"{startMenuDir}\" /A");
        RunElevatedTool("icacls.exe", $"\"{startMenuDir}\" /grant *S-1-5-32-544:F /C /Q");

        // Delete the stale .lnk after ACL reset so the new copy/move lands on a fresh
        // filename with fresh permissions.
        try
        {
            if (File.Exists(StartMenuShortcutPath))
                File.Delete(StartMenuShortcutPath);
        }
        catch { /* let the caller's move/copy throw with the real error */ }
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
