using System;

namespace Plith.Installer.Services;

/// <summary>
/// Thrown by <see cref="InstallOrchestrator"/> when repeated <c>Process.Kill</c>
/// rounds fail to make every Plith process exit — before the copy step even
/// starts. Surfaces a clear "please Exit Plith from the tray" message directly
/// in the installer UI instead of continuing into the guaranteed
/// UnauthorizedAccessException the copy step would then hit.
/// </summary>
public sealed class PlithStillRunningException : Exception
{
    public int SurvivingProcessCount { get; }

    public PlithStillRunningException(int surviving)
        : base(BuildMessage(surviving))
    {
        SurvivingProcessCount = surviving;
    }

    private static string BuildMessage(int count)
    {
        return $"Couldn't stop the running Plith ({count} process(es) still alive)." +
               " Right-click the Plith icon in the system tray and choose Exit, wait" +
               " a couple of seconds, then re-run the installer. If the tray icon" +
               " isn't visible, open Task Manager and end any 'Plith' process, then" +
               " re-run the installer.";
    }
}
