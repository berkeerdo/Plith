using System.Globalization;
using System.IO;
using IniParser;
using IniParser.Model;

namespace Plith.Services;

/// <summary>
/// INI-backed settings store at <c>%LOCALAPPDATA%\Plith\config.ini</c>.
/// Holds a live <see cref="SettingsModel"/> snapshot and raises <see cref="Changed"/>
/// after each <see cref="Save"/> so the orchestrator/window can react without restart.
/// </summary>
public sealed class SettingsService
{
    private const string SectionGeneral = "General";
    private const string SectionOsd = "Osd";
    private const string SectionAudio = "Audio";
    private const string SectionMedia = "Media";

    private readonly string _path;
    private readonly FileIniDataParser _parser = new();

    public SettingsModel Current { get; private set; } = new();

    public event Action<SettingsModel>? Changed;

    public SettingsService() : this(DefaultPath()) { }

    /// <summary>Test-friendly ctor: caller supplies the INI path explicitly.</summary>
    public SettingsService(string iniPath)
    {
        _path = iniPath;
        Directory.CreateDirectory(Path.GetDirectoryName(_path)!);
    }

    private static string DefaultPath()
    {
        var dir = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Plith");
        return Path.Combine(dir, "config.ini");
    }

    public void Load()
    {
        if (!File.Exists(_path)) { Current = new SettingsModel(); return; }
        try
        {
            var data = _parser.ReadFile(_path);
            var m = new SettingsModel
            {
                AutoStart = ParseBool(data[SectionGeneral]["AutoStart"], false),
                Theme = ParseEnum(data[SectionGeneral]["Theme"], ThemeMode.Dark),
                ShowDurationMs = ParseInt(data[SectionOsd]["ShowDurationMs"], 2000, 500, 10000),
                Position = ParseEnum(data[SectionOsd]["Position"], OsdPosition.BottomCenter),
                HoverKeepAlive = ParseBool(data[SectionOsd]["HoverKeepAlive"], true),
                OsdOpacityPercent = ParseInt(data[SectionOsd]["OsdOpacityPercent"], 100, 50, 100),
                UseColorThresholds = ParseBool(data[SectionOsd]["UseColorThresholds"], false),
                CompactMode = ParseBool(data[SectionOsd]["CompactMode"], false),
                AudioSource = ParseEnum(data[SectionAudio]["AudioSource"], AudioSourceMode.Auto),
                MonitoredBusIndex = ParseInt(data[SectionAudio]["MonitoredBusIndex"], 0, 0, 31),
                MonitoredWindowsEndpointId = data[SectionAudio]["MonitoredWindowsEndpointId"] ?? string.Empty,
                AutoShowOnMedia = ParseBool(data[SectionMedia]["AutoShowOnMedia"], false),
                SummonHotkeyMods = ParseUInt(data[SectionOsd]["SummonHotkeyMods"], 0),
                SummonHotkeyKey = ParseInt(data[SectionOsd]["SummonHotkeyKey"], 0, 0, 255),
            };
            // Migration: older config files only persisted the SummonHotkey enum string. If
            // the new raw fields are absent (still both 0) but the enum was set, translate it
            // so users don't lose their hotkey choice on upgrade.
            if (m.SummonHotkeyMods == 0 && m.SummonHotkeyKey == 0)
            {
                var (mods, vk) = HotkeyService.MigrateLegacy(data[SectionOsd]["SummonHotkey"]);
                m.SummonHotkeyMods = mods;
                m.SummonHotkeyKey = vk;
            }
            Current = m;
        }
        catch
        {
            // Corrupt INI — fall back to defaults rather than crash.
            Current = new SettingsModel();
        }
    }

    public void Save(SettingsModel m)
    {
        var data = new IniData();
        // INI persistence is locale-agnostic — config.ini written on a tr-TR machine
        // must read identically on en-US, so all numeric/bool/enum conversions use
        // CultureInfo.InvariantCulture and "G" formatting throughout.
        var inv = CultureInfo.InvariantCulture;
        data[SectionGeneral]["AutoStart"] = m.AutoStart.ToString(inv);
        data[SectionGeneral]["Theme"] = m.Theme.ToString();
        data[SectionOsd]["ShowDurationMs"] = m.ShowDurationMs.ToString(inv);
        data[SectionOsd]["Position"] = m.Position.ToString();
        data[SectionOsd]["HoverKeepAlive"] = m.HoverKeepAlive.ToString(inv);
        data[SectionOsd]["OsdOpacityPercent"] = m.OsdOpacityPercent.ToString(inv);
        data[SectionOsd]["UseColorThresholds"] = m.UseColorThresholds.ToString(inv);
        data[SectionOsd]["CompactMode"] = m.CompactMode.ToString(inv);
        data[SectionOsd]["SummonHotkeyMods"] = m.SummonHotkeyMods.ToString(inv);
        data[SectionOsd]["SummonHotkeyKey"] = m.SummonHotkeyKey.ToString(inv);
        // Strip the legacy enum key on save so we don't keep a stale value around.
        data[SectionOsd].RemoveKey("SummonHotkey");
        data[SectionAudio]["AudioSource"] = m.AudioSource.ToString();
        data[SectionAudio]["MonitoredBusIndex"] = m.MonitoredBusIndex.ToString(inv);
        data[SectionAudio]["MonitoredWindowsEndpointId"] = m.MonitoredWindowsEndpointId ?? string.Empty;
        data[SectionMedia]["AutoShowOnMedia"] = m.AutoShowOnMedia.ToString(inv);
        _parser.WriteFile(_path, data);

        Current = m.Clone();
        Changed?.Invoke(Current);
    }

    private static bool ParseBool(string? s, bool fallback)
        => bool.TryParse(s, out var v) ? v : fallback;

    private static int ParseInt(string? s, int fallback, int min, int max)
        => int.TryParse(s, NumberStyles.Integer, CultureInfo.InvariantCulture, out var v) ? Math.Clamp(v, min, max) : fallback;

    private static uint ParseUInt(string? s, uint fallback)
        => uint.TryParse(s, NumberStyles.Integer, CultureInfo.InvariantCulture, out var v) ? v : fallback;

    private static T ParseEnum<T>(string? s, T fallback) where T : struct, Enum
        => Enum.TryParse<T>(s, ignoreCase: true, out var v) ? v : fallback;
}
