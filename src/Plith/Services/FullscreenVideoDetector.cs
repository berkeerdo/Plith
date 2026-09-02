namespace Plith.Services;

/// <summary>
/// The suppression decision, isolated from every Win32 call so it can be tested exhaustively.
/// FullscreenVideoWatcher gathers the inputs; this decides.
///
/// The rule fails toward "do not hide" in every ambiguous case. A user running neither a
/// listed player nor a media-publishing app sees byte-identical 0.1.5 behaviour, and an
/// exclusive-fullscreen game can never be suppressed no matter what else matches.
/// </summary>
internal static class FullscreenVideoDetector
{
    /// <summary>SHQueryUserNotificationState: a full-screen (non-D3D) window is running.</summary>
    public const uint QUNS_BUSY = 2;

    /// <summary>SHQueryUserNotificationState: a D3D exclusive-fullscreen app is running.
    /// This is the games case and is a hard veto.</summary>
    public const uint QUNS_RUNNING_D3D_FULL_SCREEN = 3;

    public static bool ShouldSuppress(
        bool enabled,
        bool foregroundCoversMonitor,
        uint notificationState,
        bool foregroundOwnsPlayingSmtc,
        string foregroundProcessName,
        IReadOnlyCollection<string> hideList)
    {
        if (!enabled) return false;
        if (!foregroundCoversMonitor) return false;
        if (notificationState == QUNS_RUNNING_D3D_FULL_SCREEN) return false;

        return foregroundOwnsPlayingSmtc || MatchesHideList(foregroundProcessName, hideList);
    }

    private static bool MatchesHideList(string processName, IReadOnlyCollection<string> hideList)
    {
        if (string.IsNullOrWhiteSpace(processName)) return false;

        var bare = TrimExe(processName);
        foreach (var entry in hideList)
        {
            if (string.Equals(bare, TrimExe(entry), StringComparison.OrdinalIgnoreCase))
                return true;
        }
        return false;
    }

    // Process.ProcessName never carries the extension, but a user typing the hide list by hand
    // will write "mpv.exe" as often as "mpv". Normalise both sides.
    private static string TrimExe(string name) =>
        name.EndsWith(".exe", StringComparison.OrdinalIgnoreCase)
            ? name[..^4]
            : name;

    public static IReadOnlyCollection<string> ParseHideList(string? raw)
    {
        if (string.IsNullOrWhiteSpace(raw)) return Array.Empty<string>();
        return raw.Split(',', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries);
    }
}
