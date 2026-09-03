using System.IO;

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

    /// <summary>
    /// Whether the foreground process owns the AUMID reporting the media session.
    ///
    /// This lives here rather than in the watcher because it is a decision, not a gather, and
    /// because it is the predicate game safety rests on: Windows 11's Fullscreen Optimizations
    /// turn most "exclusive fullscreen" games into borderless windows reporting QUNS_BUSY, so
    /// the D3D veto never fires for them and nothing else stands between a focused game and
    /// suppression.
    ///
    /// Equality on the filename stem, never Contains. A substring test against a long packaged
    /// AUMID would accept any short or generic foreground process name that happens to appear
    /// inside it, letting a focused game be credited with a media session something else owns.
    ///
    /// What a packaged AUMID actually reduces to is worth stating precisely, because it is not
    /// "nothing". Measured on a real machine, Spotify's Store build reports
    /// "SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify", and GetFileNameWithoutExtension trims
    /// from the last dot, leaving "SpotifyAB" — short and plausible-looking, just not equal to
    /// any real process name. So packaged apps fail toward showing the OSD, which is the safe
    /// direction, but they do produce a stem rather than an empty string. Anyone relaxing this
    /// on the belief that packaged AUMIDs cannot match at all would be reasoning from a false
    /// premise.
    ///
    /// The practical consequence: fullscreen video in a packaged player is not auto-hidden,
    /// and the user's hide list is the override for it. Win32 AUMIDs ("vlc.exe", or a full
    /// path to one) reduce to the process name and match normally.
    /// </summary>
    public static bool AumidMatchesProcess(string? aumid, string? processName)
    {
        if (string.IsNullOrWhiteSpace(aumid) || string.IsNullOrWhiteSpace(processName))
            return false;

        return string.Equals(
            Path.GetFileNameWithoutExtension(aumid),
            processName,
            StringComparison.OrdinalIgnoreCase);
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
