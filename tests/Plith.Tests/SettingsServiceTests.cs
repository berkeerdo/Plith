using System.Globalization;
using System.IO;
using Plith.Services;

namespace Plith.Tests;

public class SettingsServiceTests
{
    [Fact]
    public void Load_MissingFile_ReturnsDefaults()
    {
        using var dir = new TempIniDir();
        var svc = new SettingsService(dir.IniPath);
        svc.Load();

        Assert.Equal(2000, svc.Current.ShowDurationMs);
        Assert.Equal(OsdPosition.BottomCenter, svc.Current.Position);
        Assert.True(svc.Current.HoverKeepAlive);
        Assert.Equal(AudioSourceMode.Auto, svc.Current.AudioSource);
        Assert.Equal(0, svc.Current.MonitoredBusIndex);
        Assert.False(svc.Current.AutoShowOnMedia);
        Assert.False(svc.Current.AutoStart);
        Assert.Equal((uint)0, svc.Current.SummonHotkeyMods);
        Assert.Equal(0, svc.Current.SummonHotkeyKey);
        Assert.False(svc.Current.HasSummonHotkey);
        Assert.Equal(ThemeMode.Dark, svc.Current.Theme);
    }

    [Fact]
    public void Save_Then_Load_RoundTripsAllFields()
    {
        using var dir = new TempIniDir();
        var svc = new SettingsService(dir.IniPath);

        var m = new SettingsModel
        {
            ShowDurationMs = 5000,
            Position = OsdPosition.TopRight,
            HoverKeepAlive = false,
            OsdOpacityPercent = 75,
            UseColorThresholds = true,
            CompactMode = true,
            AudioSource = AudioSourceMode.ForceVoicemeeter,
            MonitoredBusIndex = 3,
            AutoShowOnMedia = true,
            AutoStart = true,
            SummonHotkeyMods = 0x03,   // Ctrl | Alt
            SummonHotkeyKey = 0x56,    // V
            Theme = ThemeMode.Light,
        };
        svc.Save(m);

        var svc2 = new SettingsService(dir.IniPath);
        svc2.Load();

        Assert.Equal(5000, svc2.Current.ShowDurationMs);
        Assert.Equal(OsdPosition.TopRight, svc2.Current.Position);
        Assert.False(svc2.Current.HoverKeepAlive);
        Assert.Equal(75, svc2.Current.OsdOpacityPercent);
        Assert.True(svc2.Current.UseColorThresholds);
        Assert.True(svc2.Current.CompactMode);
        Assert.Equal(AudioSourceMode.ForceVoicemeeter, svc2.Current.AudioSource);
        Assert.Equal(3, svc2.Current.MonitoredBusIndex);
        Assert.True(svc2.Current.AutoShowOnMedia);
        Assert.True(svc2.Current.AutoStart);
        Assert.Equal((uint)0x03, svc2.Current.SummonHotkeyMods);
        Assert.Equal(0x56, svc2.Current.SummonHotkeyKey);
        Assert.True(svc2.Current.HasSummonHotkey);
        Assert.Equal(ThemeMode.Light, svc2.Current.Theme);
    }

    [Fact]
    public void Load_UnknownThemeValue_FallsBackToDark()
    {
        using var dir = new TempIniDir();
        File.WriteAllText(dir.IniPath, """
            [General]
            Theme = NeonRainbow
            """);
        var svc = new SettingsService(dir.IniPath);
        svc.Load();
        Assert.Equal(ThemeMode.Dark, svc.Current.Theme);
    }

    [Fact]
    public void Load_LegacyHotkeyEnum_IsMigratedToRawFields()
    {
        using var dir = new TempIniDir();
        File.WriteAllText(dir.IniPath, """
            [Osd]
            SummonHotkey = CtrlAltV
            """);
        var svc = new SettingsService(dir.IniPath);
        svc.Load();

        // CtrlAltV migrates to Ctrl|Alt (mods = 6) + V (vk = 0x56)
        Assert.Equal((uint)0x03, svc.Current.SummonHotkeyMods);
        Assert.Equal(0x56, svc.Current.SummonHotkeyKey);
    }

    [Fact]
    public void Save_ClearsLegacyHotkeyEnumKey()
    {
        using var dir = new TempIniDir();
        // Start with a legacy-style file
        File.WriteAllText(dir.IniPath, """
            [Osd]
            SummonHotkey = CtrlAltV
            """);
        var svc = new SettingsService(dir.IniPath);
        svc.Load();
        svc.Save(svc.Current.Clone());

        var ini = File.ReadAllText(dir.IniPath);
        Assert.DoesNotContain("SummonHotkey =", ini, StringComparison.Ordinal);
        Assert.Contains("SummonHotkeyMods", ini, StringComparison.Ordinal);
        Assert.Contains("SummonHotkeyKey", ini, StringComparison.Ordinal);
    }

    [Fact]
    public void Load_OpacityOutOfRange_IsClamped()
    {
        using var dir = new TempIniDir();
        File.WriteAllText(dir.IniPath, """
            [Osd]
            OsdOpacityPercent = 200
            """);
        var svc = new SettingsService(dir.IniPath);
        svc.Load();
        Assert.InRange(svc.Current.OsdOpacityPercent, 50, 100);
    }

    [Fact]
    public void Load_OpacityNegative_IsClamped()
    {
        using var dir = new TempIniDir();
        File.WriteAllText(dir.IniPath, """
            [Osd]
            OsdOpacityPercent = -5
            """);
        var svc = new SettingsService(dir.IniPath);
        svc.Load();
        Assert.InRange(svc.Current.OsdOpacityPercent, 50, 100);
    }

    [Fact]
    public void Save_OnTrTrCulture_IsLocaleIndependent()
    {
        using var dir = new TempIniDir();
        using var culture = new CulturalContext("tr-TR");

        var svc = new SettingsService(dir.IniPath);
        var m = svc.Current.Clone();
        m.ShowDurationMs = 1234;
        m.AutoStart = true;
        svc.Save(m);

        var ini = File.ReadAllText(dir.IniPath);
        Assert.Contains("ShowDurationMs = 1234", ini, StringComparison.Ordinal);
        Assert.Contains("AutoStart = True", ini, StringComparison.Ordinal);
    }

    [Fact]
    public void Load_CorruptIni_FallsBackToDefaults()
    {
        using var dir = new TempIniDir();
        File.WriteAllText(dir.IniPath, "not actually ini\nfile  ===\n[broken");

        var svc = new SettingsService(dir.IniPath);
        svc.Load();
        Assert.Equal(2000, svc.Current.ShowDurationMs);
        Assert.Equal(OsdPosition.BottomCenter, svc.Current.Position);
    }

    [Fact]
    public void Load_OutOfRangeValues_AreClamped()
    {
        using var dir = new TempIniDir();
        File.WriteAllText(dir.IniPath, """
            [Osd]
            ShowDurationMs = 99999

            [Audio]
            MonitoredBusIndex = -5
            """);

        var svc = new SettingsService(dir.IniPath);
        svc.Load();
        Assert.InRange(svc.Current.ShowDurationMs, 500, 10000);
        Assert.InRange(svc.Current.MonitoredBusIndex, 0, 31);
    }

    [Fact]
    public void Save_RaisesChangedEvent_WithUpdatedSnapshot()
    {
        using var dir = new TempIniDir();
        var svc = new SettingsService(dir.IniPath);
        SettingsModel? snapshot = null;
        svc.Changed += m => snapshot = m;

        var modified = svc.Current.Clone();
        modified.ShowDurationMs = 7777;
        svc.Save(modified);

        Assert.NotNull(snapshot);
        Assert.Equal(7777, snapshot!.ShowDurationMs);
        Assert.NotSame(modified, snapshot);
    }

    [Fact]
    public void Clone_ProducesIndependentCopy()
    {
        var m = new SettingsModel { ShowDurationMs = 3000, AutoStart = true };
        var c = m.Clone();
        c.ShowDurationMs = 9999;
        Assert.Equal(3000, m.ShowDurationMs);
        Assert.True(c.AutoStart);
    }

    [Fact]
    public void Load_MissingFile_DefaultsAccentToEmerald()
    {
        using var dir = new TempIniDir();
        var svc = new SettingsService(dir.IniPath);
        svc.Load();

        Assert.Equal(AccentTheme.DefaultId, svc.Current.AccentThemeId);
        Assert.Null(svc.Current.CustomAccentColor);
    }

    [Fact]
    public void Save_Then_Load_RoundTripsAccentPreset()
    {
        using var dir = new TempIniDir();
        var svc = new SettingsService(dir.IniPath);
        var m = svc.Current.Clone();
        m.AccentThemeId = "lime";
        svc.Save(m);

        var svc2 = new SettingsService(dir.IniPath);
        svc2.Load();
        Assert.Equal("lime", svc2.Current.AccentThemeId);
        Assert.Null(svc2.Current.CustomAccentColor);
    }

    [Fact]
    public void Save_Then_Load_RoundTripsCustomAccent()
    {
        using var dir = new TempIniDir();
        var svc = new SettingsService(dir.IniPath);
        var m = svc.Current.Clone();
        m.AccentThemeId = AccentTheme.CustomId;
        m.CustomAccentColor = "#7AA2F7";
        svc.Save(m);

        var svc2 = new SettingsService(dir.IniPath);
        svc2.Load();
        Assert.Equal(AccentTheme.CustomId, svc2.Current.AccentThemeId);
        Assert.Equal("#7AA2F7", svc2.Current.CustomAccentColor);
    }

    [Fact]
    public void Save_KeepsCustomHex_EvenWhenPresetIsActive()
    {
        // Design choice: switching to a preset must not erase the last custom colour.
        // The popup re-opens on the last picked hex when the user returns to Custom.
        using var dir = new TempIniDir();
        var svc = new SettingsService(dir.IniPath);
        var m = svc.Current.Clone();
        m.AccentThemeId = "lime";
        m.CustomAccentColor = "#CAFF33";
        svc.Save(m);

        var svc2 = new SettingsService(dir.IniPath);
        svc2.Load();
        Assert.Equal("lime", svc2.Current.AccentThemeId);
        Assert.Equal("#CAFF33", svc2.Current.CustomAccentColor);
    }

    [Fact]
    public void Load_UnknownAccentId_IsAcceptedVerbatim()
    {
        // Ids from future / older builds should survive a round-trip without being
        // coerced to Emerald — the picker just won't show a selection ring for them,
        // and ThemeService.ResolveBase falls back to Emerald at apply-time.
        using var dir = new TempIniDir();
        File.WriteAllText(dir.IniPath, """
            [Appearance]
            AccentThemeId = future-preset-99
            """);
        var svc = new SettingsService(dir.IniPath);
        svc.Load();
        Assert.Equal("future-preset-99", svc.Current.AccentThemeId);
    }
}

internal sealed class TempIniDir : IDisposable
{
    public string IniPath { get; }
    private readonly string _dir;

    public TempIniDir()
    {
        _dir = Path.Combine(Path.GetTempPath(), "PlithTests-" + Guid.NewGuid());
        Directory.CreateDirectory(_dir);
        IniPath = Path.Combine(_dir, "config.ini");
    }

    public void Dispose()
    {
        try { Directory.Delete(_dir, recursive: true); } catch { }
    }
}

internal sealed class CulturalContext : IDisposable
{
    private readonly CultureInfo _original;
    public CulturalContext(string name)
    {
        _original = CultureInfo.CurrentCulture;
        CultureInfo.CurrentCulture = new CultureInfo(name);
    }
    public void Dispose() => CultureInfo.CurrentCulture = _original;
}
