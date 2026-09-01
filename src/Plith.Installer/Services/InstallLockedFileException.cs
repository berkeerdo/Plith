using System;

namespace Plith.Installer.Services;

/// <summary>
/// Thrown by <see cref="InstallOrchestrator.CopyWithRetry"/> when both the retry loop
/// AND the sideline-rename fallback fail to overwrite a target file. The message is
/// tuned for direct display in the installer UI — it names the file, states the two
/// most common root causes, and hands the user a concrete fix path.
///
/// Kept as its own type so the UI layer can distinguish this "user-actionable" failure
/// from a generic exception and, in future, surface a dedicated "Retry" button that
/// waits longer instead of re-throwing to the failure screen.
/// </summary>
public sealed class InstallLockedFileException : Exception
{
    public string LockedFilePath { get; }

    public InstallLockedFileException(string lockedFilePath, Exception inner)
        : base(BuildMessage(lockedFilePath), inner)
    {
        LockedFilePath = lockedFilePath;
    }

    private static string BuildMessage(string path)
    {
        return $"Couldn't overwrite '{path}' after retrying and moving the old copy aside." +
               " Plith is probably still holding it — right-click the Plith icon in the" +
               " system tray and choose Exit, then re-run the installer. If the problem" +
               " keeps happening, add %ProgramFiles%\\Plith to your antivirus scan" +
               " exclusions (Norton: Settings > Antivirus > Scans and Risks > Items to" +
               " Exclude from Scans + Items to Exclude from Auto-Protect).";
    }
}
