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

// Legacy enum kept only to migrate INI files that pre-date the free-form capture UI.
// New code uses raw (mods, vk) on SettingsModel instead.
internal enum LegacyHotkeyCombo
{
    None,
    CtrlAltV,
    CtrlShiftV,
    AltShiftV,
    CtrlAltM,
}

public sealed class SettingsModel
{
    /// <summary>How long the OSD stays visible after the last change, in milliseconds.</summary>
    public int ShowDurationMs { get; set; } = 2000;

    /// <summary>Where on the primary screen the OSD anchors.</summary>
    public OsdPosition Position { get; set; } = OsdPosition.BottomCenter;

    /// <summary>If true, a media event (track change, play/pause from the source app) pops the OSD.
    /// Default off — surfacing every Spotify advance is intrusive.</summary>
    public bool AutoShowOnMedia { get; set; }

    /// <summary>If true, mouse over the OSD pauses the hide timer.</summary>
    public bool HoverKeepAlive { get; set; } = true;

    /// <summary>OSD card opacity at rest, 50–100 percent. Below 50 the OSD is hard to read.</summary>
    public int OsdOpacityPercent { get; set; } = 100;

    /// <summary>If true, the volume bar colour reflects level — green at safe levels, amber
    /// near the top, red into the "too loud" zone. Off by default since the constant accent
    /// green is calmer and consistent with the rest of the UI.</summary>
    public bool UseColorThresholds { get; set; }

    /// <summary>If true, the OSD never shows the media card, even when a SMTC session is
    /// active. Useful on small or vertical screens, or for users who only want the volume
    /// part. Off by default.</summary>
    public bool CompactMode { get; set; }

    /// <summary>How Plith picks between Voicemeeter and the Windows default endpoint.</summary>
    public AudioSourceMode AudioSource { get; set; } = AudioSourceMode.Auto;

    /// <summary>Index of the Voicemeeter bus to monitor (0 = A1, 1 = A2, etc.). Only used when the active source is Voicemeeter.</summary>
    public int MonitoredBusIndex { get; set; }

    /// <summary>If true, a registry Run entry launches Plith on Windows login.</summary>
    public bool AutoStart { get; set; }

    /// <summary>Bitmask of modifier keys for the summon hotkey. 0 = no hotkey bound.
    /// Bit layout matches the RegisterHotKey API: Alt=1, Ctrl=2, Shift=4, Win=8.</summary>
    public uint SummonHotkeyMods { get; set; }

    /// <summary>Virtual-key code for the summon hotkey (e.g. 0x56 = 'V', 0x70 = F1).
    /// 0 = no hotkey bound. Both this and <see cref="SummonHotkeyMods"/> must be non-zero
    /// for the hotkey to register.</summary>
    public int SummonHotkeyKey { get; set; }

    /// <summary>True when both the modifier mask and the virtual key are set.</summary>
    public bool HasSummonHotkey => SummonHotkeyMods != 0 && SummonHotkeyKey != 0;

    public SettingsModel Clone() => new()
    {
        ShowDurationMs = ShowDurationMs,
        Position = Position,
        AutoShowOnMedia = AutoShowOnMedia,
        HoverKeepAlive = HoverKeepAlive,
        OsdOpacityPercent = OsdOpacityPercent,
        UseColorThresholds = UseColorThresholds,
        CompactMode = CompactMode,
        AudioSource = AudioSource,
        MonitoredBusIndex = MonitoredBusIndex,
        AutoStart = AutoStart,
        SummonHotkeyMods = SummonHotkeyMods,
        SummonHotkeyKey = SummonHotkeyKey,
    };
}
