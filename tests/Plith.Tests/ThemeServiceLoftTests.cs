using System.Windows;
using System.Windows.Media;
using Plith.Services;

namespace Plith.Tests;

public class ThemeServiceLoftTests
{
    private static readonly Color[] Accents =
    [
        Color.FromRgb(0xCA, 0xFF, 0x33),
        Color.FromRgb(0x6D, 0x28, 0xD9),
        Color.FromRgb(0x4A, 0xD6, 0x95),
    ];

    /// <summary>
    /// A runtime theme switch reaches the notch only through the dictionary OsdHost mirrors, so
    /// every key the OSD palette defines must be reachable through it, with the palette's own
    /// value unless the accent override deliberately supersedes it. Enumerates the real palette
    /// files, so a token added to one is covered without editing this test.
    /// </summary>
    [StaTheory]
    [InlineData("OsdPalette.Dark.xaml", true)]
    [InlineData("OsdPalette.Light.xaml", false)]
    public void OsdMirror_CarriesEveryKeyTheOsdPaletteDefines(string file, bool isDark)
    {
        var palette = (ResourceDictionary)Application.LoadComponent(
            new Uri($"/Plith;component/Resources/{file}", UriKind.Relative));
        var accent = ThemeService.BuildAccentOverride(Accents[1], isDark);

        var mirror = ThemeService.BuildOsdMirror(palette, accent);

        Assert.NotEmpty(palette.Keys);
        foreach (var key in palette.Keys)
        {
            Assert.True(mirror.Contains(key), $"{key} from {file} is not reachable through the OSD mirror");
            var expected = accent.Contains(key) ? accent[key] : palette[key];
            Assert.Same(expected, mirror[key]);
        }
        foreach (var key in accent.Keys) Assert.True(mirror.Contains(key), $"{key} lost from the accent override");
        foreach (var key in new[] { "NotchInk", "NotchInkMuted", "NotchTrack" })
            Assert.Same(palette[key], mirror[key]);
    }

    private static readonly object AppLock = new();

    /// <summary>
    /// The LIVE path: the mirror must follow the palette a theme switch installs, so it has to be
    /// read from the current slot on every call. Caching the palette at construction would pass
    /// the static composition test above and still strand the ink after a Dark to Light switch.
    /// </summary>
    [StaFact]
    public void OsdMirror_FollowsTheActivePaletteAcrossAThemeSwitch()
    {
        Application app;
        lock (AppLock)
        {
            app = Application.Current ?? new Application();
            if (app.Resources.MergedDictionaries.Count == 0)
            {
                foreach (var name in new[] { "OsdPalette.Dark.xaml", "Palette.Dark.xaml" })
                    app.Resources.MergedDictionaries.Add(new ResourceDictionary
                    {
                        Source = new Uri($"pack://application:,,,/Plith;component/Resources/{name}", UriKind.Absolute),
                    });
            }
        }

        var ini = System.IO.Path.Combine(System.IO.Path.GetTempPath(), "plith-theme-" + Guid.NewGuid().ToString("N"), "config.ini");
        var theme = new ThemeService(app, new SettingsService(ini));
        if (theme.IsHighContrast) return;

        theme.Apply(Plith.Services.ThemeMode.Dark);
        var dark = Assert.IsType<SolidColorBrush>(theme.BuildOsdMirror()["NotchInk"]).Color;
        theme.Apply(Plith.Services.ThemeMode.Light);
        var light = Assert.IsType<SolidColorBrush>(theme.BuildOsdMirror()["NotchInk"]).Color;

        Assert.Equal(LoftPalette.DarkFloor.Foreground, dark);
        Assert.Equal(LoftPalette.LightFloor.Foreground, light);
        Assert.NotEqual(dark, light);
    }

    [Fact]
    public void BuildAccentOverride_SurfaceIsTheFloorAndNotTheAccent()
    {
        foreach (var isDark in new[] { true, false })
        {
            var floor = LoftPalette.For(isDark);

            foreach (var accent in Accents)
            {
                var dict = ThemeService.BuildAccentOverride(accent, isDark);
                var brush = Assert.IsType<LinearGradientBrush>(dict["OsdSurfaceBrush"]);

                // The floor's colour, at the OSD's long-standing slight translucency.
                var alpha = isDark ? (byte)0xF0 : (byte)0xFA;
                Assert.Equal(Color.FromArgb(alpha, floor.Background.R, floor.Background.G, floor.Background.B),
                    brush.GradientStops[0].Color);
                Assert.Equal(Color.FromArgb(alpha, floor.Surface.R, floor.Surface.G, floor.Surface.B),
                    brush.GradientStops[1].Color);
            }
        }
    }

    [Fact]
    public void BuildAccentOverride_KeySetIsStableAcrossThemesAndAccents()
    {
        var expected = ThemeService.BuildAccentOverride(Accents[0], isDark: true).Keys
            .Cast<object>().Select(k => k.ToString()).OrderBy(k => k).ToArray();

        // Named, so a stub returning {} cannot satisfy the comparison below.
        Assert.Equal(
            new[]
            {
                "Accent", "AccentGlow", "AccentHover", "AccentPressed", "AccentTrack", "OnAccent",
                "OsdAccent", "OsdAccentMark", "OsdAccentText", "OsdBorder", "OsdDivider",
                "OsdSurfaceBrush", "OsdTrackBg",
            },
            expected);

        foreach (var isDark in new[] { true, false })
        {
            foreach (var accent in Accents)
            {
                var keys = ThemeService.BuildAccentOverride(accent, isDark).Keys
                    .Cast<object>().Select(k => k.ToString()).OrderBy(k => k).ToArray();

                Assert.Equal(expected, keys);
            }
        }
    }

    [Fact]
    public void BuildAccentOverride_DropsTheKeysTheFixedFloorMakesConstant()
    {
        var dict = ThemeService.BuildAccentOverride(Accents[0], isDark: true);

        Assert.False(dict.Contains("NotchInk"));
        Assert.False(dict.Contains("NotchInkMuted"));
        Assert.False(dict.Contains("NotchTrack"));
    }

    [Fact]
    public void BuildAccentOverride_KeepsAccentTrackForTheSettingsProgressBar()
    {
        // Correction 1: AccentTrack lives on the Settings path (out of scope) and keeps its
        // accent-derived value, or the Updates card bar disappears on a deep accent.
        foreach (var isDark in new[] { true, false })
        {
            foreach (var accent in Accents)
            {
                var dict = ThemeService.BuildAccentOverride(accent, isDark);
                var track = Assert.IsType<SolidColorBrush>(dict["AccentTrack"]);
                // The motivating property: the groove must be distinguishable from the Settings
                // accent that fills it, which a general neutral was not (1.6:1 on a deep accent).
                var derived = AccentTheme.Derive(accent, isDark).Accent;

                Assert.True(ContrastInk.ContrastRatio(track.Color, derived) >= 3.0,
                    $"{accent} isDark={isDark}: groove and fill nearly identical");
            }
        }
    }

    [Fact]
    public void BuildAccentOverride_OsdAccentClearsThreeToOneOnTheFloor()
    {
        foreach (var isDark in new[] { true, false })
        {
            var floor = LoftPalette.For(isDark);

            foreach (var accent in AccentTheme.Presets.Select(p => p.BaseColor).Concat(Accents))
            {
                var bar = Assert.IsType<SolidColorBrush>(ThemeService.BuildAccentOverride(accent, isDark)["OsdAccent"]);

                foreach (var ground in new[] { floor.Background, floor.Surface })
                {
                    Assert.True(ContrastInk.ContrastRatio(bar.Color, ground) >= 3.0,
                        $"{accent} isDark={isDark}: OsdAccent below 3:1 on {ground}");
                }
            }
        }
    }

    [Fact]
    public void BuildAccentOverride_CarriesTheMarkAndTextRoles()
    {
        foreach (var isDark in new[] { true, false })
        {
            var floor = LoftPalette.For(isDark);

            foreach (var accent in Accents)
            {
                var dict = ThemeService.BuildAccentOverride(accent, isDark);
                var mark = Assert.IsType<SolidColorBrush>(dict["OsdAccentMark"]);
                var text = Assert.IsType<SolidColorBrush>(dict["OsdAccentText"]);

                // Both surfaces, as DeriveLoftRoles guarantees and the notch draws on both.
                foreach (var ground in new[] { floor.Background, floor.Surface })
                {
                    Assert.True(ContrastInk.ContrastRatio(mark.Color, ground) >= 3.0);
                    Assert.True(ContrastInk.ContrastRatio(text.Color, ground) >= 4.5);
                }
            }
        }
    }
}
