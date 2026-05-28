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

    public SettingsService()
    {
        var dir = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Plith");
        Directory.CreateDirectory(dir);
        _path = Path.Combine(dir, "config.ini");
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
                ShowDurationMs = ParseInt(data[SectionOsd]["ShowDurationMs"], 2000, 500, 10000),
                Position = ParseEnum(data[SectionOsd]["Position"], OsdPosition.BottomCenter),
                HoverKeepAlive = ParseBool(data[SectionOsd]["HoverKeepAlive"], true),
                AudioSource = ParseEnum(data[SectionAudio]["AudioSource"], AudioSourceMode.Auto),
                MonitoredBusIndex = ParseInt(data[SectionAudio]["MonitoredBusIndex"], 0, 0, 31),
                AutoShowOnMedia = ParseBool(data[SectionMedia]["AutoShowOnMedia"], false),
                SummonHotkey = ParseEnum(data[SectionOsd]["SummonHotkey"], HotkeyCombo.None),
            };
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
        data[SectionGeneral]["AutoStart"] = m.AutoStart.ToString();
        data[SectionOsd]["ShowDurationMs"] = m.ShowDurationMs.ToString();
        data[SectionOsd]["Position"] = m.Position.ToString();
        data[SectionOsd]["HoverKeepAlive"] = m.HoverKeepAlive.ToString();
        data[SectionOsd]["SummonHotkey"] = m.SummonHotkey.ToString();
        data[SectionAudio]["AudioSource"] = m.AudioSource.ToString();
        data[SectionAudio]["MonitoredBusIndex"] = m.MonitoredBusIndex.ToString();
        data[SectionMedia]["AutoShowOnMedia"] = m.AutoShowOnMedia.ToString();
        _parser.WriteFile(_path, data);

        Current = m.Clone();
        Changed?.Invoke(Current);
    }

    private static bool ParseBool(string? s, bool fallback)
        => bool.TryParse(s, out var v) ? v : fallback;

    private static int ParseInt(string? s, int fallback, int min, int max)
        => int.TryParse(s, out var v) ? Math.Clamp(v, min, max) : fallback;

    private static T ParseEnum<T>(string? s, T fallback) where T : struct, Enum
        => Enum.TryParse<T>(s, ignoreCase: true, out var v) ? v : fallback;
}
