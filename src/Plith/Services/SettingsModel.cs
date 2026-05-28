namespace Plith.Services;

public enum OsdPosition
{
    BottomCenter,
    BottomRight,
    TopCenter,
    TopRight,
}

public enum AudioSourceMode
{
    /// <summary>Prefer Voicemeeter when it's running, fall back to the Windows default endpoint.</summary>
    Auto,
    /// <summary>Always use Voicemeeter; show nothing if Voicemeeter is not running.</summary>
    ForceVoicemeeter,
    /// <summary>Always use the Windows default endpoint, ignoring Voicemeeter.</summary>
    ForceWindows,
}

public sealed class SettingsModel
{
    /// <summary>How long the OSD stays visible after the last change, in milliseconds.</summary>
    public int ShowDurationMs { get; set; } = 2000;

    /// <summary>Where on the primary screen the OSD anchors.</summary>
    public OsdPosition Position { get; set; } = OsdPosition.BottomCenter;

    /// <summary>If true, a media event (track change, play/pause from the source app) pops the OSD.
    /// Default off — surfacing every Spotify advance is intrusive.</summary>
    public bool AutoShowOnMedia { get; set; } = false;

    /// <summary>If true, mouse over the OSD pauses the hide timer.</summary>
    public bool HoverKeepAlive { get; set; } = true;

    /// <summary>How Plith picks between Voicemeeter and the Windows default endpoint.</summary>
    public AudioSourceMode AudioSource { get; set; } = AudioSourceMode.Auto;

    /// <summary>Index of the Voicemeeter bus to monitor (0 = A1, 1 = A2, etc.). Only used when the active source is Voicemeeter.</summary>
    public int MonitoredBusIndex { get; set; } = 0;

    /// <summary>If true, a registry Run entry launches Plith on Windows login.</summary>
    public bool AutoStart { get; set; } = false;

    public SettingsModel Clone() => new()
    {
        ShowDurationMs = ShowDurationMs,
        Position = Position,
        AutoShowOnMedia = AutoShowOnMedia,
        HoverKeepAlive = HoverKeepAlive,
        AudioSource = AudioSource,
        MonitoredBusIndex = MonitoredBusIndex,
        AutoStart = AutoStart,
    };
}
