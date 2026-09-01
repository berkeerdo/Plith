using System;
using System.Collections.Generic;

namespace Plith.Installer.Services;

/// <summary>
/// Thrown by <see cref="InstallOrchestrator.CopyWithRetry"/> when both the retry loop
/// AND the sideline-rename fallback fail to overwrite a target file. The message is
/// tuned for direct display in the installer UI — it names the file, names the
/// processes Windows Restart Manager identified as holding it (when it could), and
/// hands the user a concrete fix path.
///
/// Kept as its own type so the UI layer can distinguish this "user-actionable" failure
/// from a generic exception and, in future, surface a dedicated "Retry" button that
/// waits longer instead of re-throwing to the failure screen.
/// </summary>
public sealed class InstallLockedFileException : Exception
{
    public string LockedFilePath { get; }
    public IReadOnlyList<string> Holders { get; }
    public string Win32ErrorDescription { get; }

    public InstallLockedFileException(
        string lockedFilePath,
        IReadOnlyList<string> holders,
        string win32ErrorDescription,
        Exception inner)
        : base(BuildMessage(lockedFilePath, holders, win32ErrorDescription), inner)
    {
        LockedFilePath = lockedFilePath;
        Holders = holders;
        Win32ErrorDescription = win32ErrorDescription;
    }

    // Convenience overloads for call sites that couldn't collect holders / Win32 code.
    public InstallLockedFileException(string lockedFilePath, IReadOnlyList<string> holders, Exception inner)
        : this(lockedFilePath, holders, string.Empty, inner) { }

    public InstallLockedFileException(string lockedFilePath, Exception inner)
        : this(lockedFilePath, Array.Empty<string>(), string.Empty, inner) { }

    private static string BuildMessage(string path, IReadOnlyList<string> holders, string win32Error)
    {
        // Holder clause tells the user WHO to close. Win32 clause tells them WHY it
        // failed even when nobody is holding it (permissions, read-only, missing dir).
        string holderClause = holders.Count switch
        {
            0 when !string.IsNullOrEmpty(win32Error) && win32Error.Contains("ACCESS_DENIED", StringComparison.Ordinal)
                => " Windows reports the file is protected — a previous install or an antivirus" +
                   " quarantine probably locked its permissions. Take ownership from an admin cmd:" +
                   $" `takeown /f \"{path}\" && icacls \"{path}\" /grant Administrators:F` then re-run.",
            0 => " Nobody appears to be holding it — the file is protected at the filesystem level.",
            1 => $" '{holders[0]}' has it open.",
            _ => $" These processes have it open: {string.Join(", ", holders)}.",
        };

        string win32Clause = string.IsNullOrEmpty(win32Error)
            ? string.Empty
            : $" Underlying error: {win32Error}.";

        return $"Couldn't overwrite '{path}' after retrying and moving the old copy aside." +
               holderClause + win32Clause +
               " Right-click the Plith icon in the system tray and choose Exit if it's" +
               " still running, then re-run the installer. If the problem keeps happening," +
               " add %ProgramFiles%\\Plith to your antivirus scan exclusions (Norton:" +
               " Settings > Antivirus > Scans and Risks > Items to Exclude from Scans +" +
               " Items to Exclude from Auto-Protect).";
    }
}
