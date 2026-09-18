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

    // Section 6.7 of the manual verification list asks a human to clear the fullscreen hide
    // list in Settings, restart, and confirm it stays empty. That is a round-trip through the
    // INI, which is testable, so it does not need a person. The failure it guards against is
    // specific: FullscreenVideoHideList has a non-empty default, so anything that treats an
    // empty stored value as "absent" silently restores "mpv,PotPlayerMini64" and the user's
    // deliberate choice is undone on the next launch.
    [Fact]
    public void Save_Then_Load_KeepsAnEmptyHideListEmpty()
    {
        using var dir = new TempIniDir();

        var write = new SettingsService(dir.IniPath);
        write.Load();
        Assert.Equal("mpv,PotPlayerMini64", write.Current.FullscreenVideoHideList);

        var cleared = write.Current.Clone();
        cleared.FullscreenVideoHideList = string.Empty;
        write.Save(cleared);

        var read = new SettingsService(dir.IniPath);
        read.Load();

        Assert.Equal(string.Empty, read.Current.FullscreenVideoHideList);
        Assert.Empty(FullscreenVideoDetector.ParseHideList(read.Current.FullscreenVideoHideList));
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

    [Fact]
    public void FullscreenVideoSettings_RoundTripThroughIni()
    {
        using var dir = new TempIniDir();
        var svc = new SettingsService(dir.IniPath);

        var m = svc.Current.Clone();
        m.HideDuringFullscreenVideo = false;
        m.FullscreenVideoHideList = "mpv,vlc";
        svc.Save(m);

        var reloaded = new SettingsService(dir.IniPath);
        reloaded.Load();

        Assert.False(reloaded.Current.HideDuringFullscreenVideo);
        Assert.Equal("mpv,vlc", reloaded.Current.FullscreenVideoHideList);
    }

    [Fact]
    public void FullscreenVideoSettings_DefaultToEnabledWithSeededHideList()
    {
        using var dir = new TempIniDir();
        var svc = new SettingsService(dir.IniPath);
        svc.Load();   // no file on disk -> defaults

        Assert.True(svc.Current.HideDuringFullscreenVideo);
        Assert.Equal("mpv,PotPlayerMini64", svc.Current.FullscreenVideoHideList);
    }

    [Fact]
    public void Load_ConfigWithoutPresentationKeys_DefaultsToClassic()
    {
        // Every existing 0.1.5 install has a config.ini with no [Osd] Presentation key.
        // They must keep the OSD they already have rather than being moved to a notch.
        var path = Path.Combine(Path.GetTempPath(), "PlithTests", Guid.NewGuid().ToString("N"), "config.ini");
        Directory.CreateDirectory(Path.GetDirectoryName(path)!);
        File.WriteAllText(path, "[Osd]\nShowDurationMs=2000\n");

        var svc = new SettingsService(path);
        svc.Load();

        Assert.Equal(PresentationMode.ClassicOsd, svc.Current.Presentation);
        Assert.Equal(5, svc.Current.NotchStripHeightDip);
    }

    [Fact]
    public void Save_Then_Load_RoundTripsPresentationSettings()
    {
        var path = Path.Combine(Path.GetTempPath(), "PlithTests", Guid.NewGuid().ToString("N"), "config.ini");
        var write = new SettingsService(path);
        var m = write.Current.Clone();
        m.Presentation = PresentationMode.AmbientNotch;
        m.NotchStripHeightDip = 6;
        write.Save(m);

        var read = new SettingsService(path);
        read.Load();

        Assert.Equal(PresentationMode.AmbientNotch, read.Current.Presentation);
        Assert.Equal(6, read.Current.NotchStripHeightDip);
    }

    [Fact]
    public void Load_ClampsAnAbsurdStripHeight()
    {
        // A hand-edited config must not be able to park a 900 px "strip" across the screen.
        var path = Path.Combine(Path.GetTempPath(), "PlithTests", Guid.NewGuid().ToString("N"), "config.ini");
        Directory.CreateDirectory(Path.GetDirectoryName(path)!);
        File.WriteAllText(path, "[Osd]\nNotchStripHeightDip=900\n");

        var svc = new SettingsService(path);
        svc.Load();

        Assert.Equal(24, svc.Current.NotchStripHeightDip);
    }

    /// <summary>
    /// The covering-window fallback round-trips, and defaults to off.
    ///
    /// The default is the half worth asserting. It used to be the only behaviour and was not a
    /// setting at all, so "off" here is the statement that a covering window no longer takes the
    /// notch away by itself — a regression to the old behaviour would show up as this failing
    /// rather than as a bug report weeks later.
    /// </summary>
    [Fact]
    public void Save_Then_Load_RoundTripsClassicOverFullscreen()
    {
        using var dir = new TempIniDir();
        var svc = new SettingsService(dir.IniPath);

        Assert.False(svc.Current.UseClassicOverFullscreen);

        var m = svc.Current.Clone();
        m.UseClassicOverFullscreen = true;
        svc.Save(m);

        var reloaded = new SettingsService(dir.IniPath);
        reloaded.Load();

        Assert.True(reloaded.Current.UseClassicOverFullscreen);
    }

    [Fact]
    public void Save_Then_Load_RoundTripsWeatherSettings()
    {
        using var dir = new TempIniDir();
        var svc = new SettingsService(dir.IniPath);

        var m = svc.Current.Clone();
        m.ShowWeather = false;
        m.WeatherLocation = "Istanbul";
        m.WeatherLatitude = 41.0082;
        m.WeatherLongitude = 28.9784;
        svc.Save(m);

        var reloaded = new SettingsService(dir.IniPath);
        reloaded.Load();

        Assert.False(reloaded.Current.ShowWeather);
        Assert.Equal("Istanbul", reloaded.Current.WeatherLocation);
        Assert.Equal(41.0082, reloaded.Current.WeatherLatitude, precision: 4);
        Assert.Equal(28.9784, reloaded.Current.WeatherLongitude, precision: 4);
    }

    [Fact]
    public void Save_NullWeatherLocation_DoesNotThrowAndRoundTripsAsEmpty()
    {
        // Regression: unlike FullscreenVideoHideList's save line, WeatherLocation's first
        // draft was missing the "?? string.Empty" guard every other string save on this page
        // has. A null here (e.g. from a future Settings binding) must not blow up Save.
        using var dir = new TempIniDir();
        var svc = new SettingsService(dir.IniPath);

        var m = svc.Current.Clone();
        m.WeatherLocation = null!;
        svc.Save(m);

        var reloaded = new SettingsService(dir.IniPath);
        reloaded.Load();

        Assert.Equal(string.Empty, reloaded.Current.WeatherLocation);
    }

    [Fact]
    public void BrightnessSettingsRoundTrip()
    {
        var path = Path.Combine(Path.GetTempPath(), "PlithTests", Guid.NewGuid().ToString("N"), "config.ini");
        var svc = new SettingsService(path);

        var m = svc.Current.Clone();
        m.BrightnessEnabled = true;
        m.BrightnessStepPercent = 15;
        m.BrightnessUpHotkeyMods = 3;
        m.BrightnessUpHotkeyKey = 0x26;
        m.BrightnessDownHotkeyMods = 3;
        m.BrightnessDownHotkeyKey = 0x28;
        svc.Save(m);

        var reader = new SettingsService(path);
        reader.Load();
        var reloaded = reader.Current;

        Assert.True(reloaded.BrightnessEnabled);
        Assert.Equal(15, reloaded.BrightnessStepPercent);
        Assert.Equal(3u, reloaded.BrightnessUpHotkeyMods);
        Assert.Equal(0x26, reloaded.BrightnessUpHotkeyKey);
        Assert.Equal(3u, reloaded.BrightnessDownHotkeyMods);
        Assert.Equal(0x28, reloaded.BrightnessDownHotkeyKey);
        Assert.True(reloaded.HasBrightnessHotkeys);
    }

    [Fact]
    public void BrightnessIsOffAndUnboundOnAFreshConfig()
    {
        var path = Path.Combine(Path.GetTempPath(), "PlithTests", Guid.NewGuid().ToString("N"), "config.ini");
        var svc = new SettingsService(path);
        svc.Load();
        var m = svc.Current;

        Assert.False(m.BrightnessEnabled);
        Assert.Equal(0u, m.BrightnessUpHotkeyMods);
        Assert.Equal(0, m.BrightnessUpHotkeyKey);
        Assert.Equal(10, m.BrightnessStepPercent);
        Assert.False(m.HasBrightnessHotkeys);
    }

    [Fact]
    public void OneDirectionBoundIsNotEnough()
    {
        // A brightness control that can only go down is worse than none.
        var path = Path.Combine(Path.GetTempPath(), "PlithTests", Guid.NewGuid().ToString("N"), "config.ini");
        var svc = new SettingsService(path);

        var m = svc.Current.Clone();
        m.BrightnessUpHotkeyMods = 3;
        m.BrightnessUpHotkeyKey = 0x26;
        svc.Save(m);

        var reader = new SettingsService(path);
        reader.Load();
        Assert.False(reader.Current.HasBrightnessHotkeys);
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
