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

                Assert.Equal(floor.Background, brush.GradientStops[0].Color);
                Assert.Equal(floor.Surface, brush.GradientStops[1].Color);
            }
        }
    }

    [Fact]
    public void BuildAccentOverride_KeySetIsStableAcrossThemesAndAccents()
    {
        var expected = ThemeService.BuildAccentOverride(Accents[0], isDark: true).Keys
            .Cast<object>().Select(k => k.ToString()).OrderBy(k => k).ToArray();

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
                var derived = AccentTheme.Derive(accent, isDark).Accent;

                Assert.Equal(ContrastInk.TrackOn(derived), track.Color);
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

                Assert.True(ContrastInk.ContrastRatio(mark.Color, floor.Surface) >= 3.0);
                Assert.True(ContrastInk.ContrastRatio(text.Color, floor.Surface) >= 4.5);
            }
        }
    }
}
