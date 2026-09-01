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

    public InstallLockedFileException(string lockedFilePath, IReadOnlyList<string> holders, Exception inner)
        : base(BuildMessage(lockedFilePath, holders), inner)
    {
        LockedFilePath = lockedFilePath;
        Holders = holders;
    }

    // Convenience overload for call sites that couldn't query Restart Manager.
    public InstallLockedFileException(string lockedFilePath, Exception inner)
        : this(lockedFilePath, Array.Empty<string>(), inner) { }

    private static string BuildMessage(string path, IReadOnlyList<string> holders)
    {
        string holderClause = holders.Count switch
        {
            0 => " Plith is probably still holding it.",
            1 => $" '{holders[0]}' has it open.",
            _ => $" These processes have it open: {string.Join(", ", holders)}.",
        };

        return $"Couldn't overwrite '{path}' after retrying and moving the old copy aside." +
               holderClause +
               " Right-click the Plith icon in the system tray and choose Exit if it's" +
               " still running, then re-run the installer. If the problem keeps happening," +
               " add %ProgramFiles%\\Plith to your antivirus scan exclusions (Norton:" +
               " Settings > Antivirus > Scans and Risks > Items to Exclude from Scans +" +
               " Items to Exclude from Auto-Protect).";
    }
}
