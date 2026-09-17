using System.Windows.Media;
using Plith.Services;

namespace Plith.Tests;

public class AccentThemeTests
{
    [Fact]
    public void Presets_IncludeExpectedDefault()
    {
        Assert.Contains(AccentTheme.Presets, p => p.Id == AccentTheme.DefaultId);
        // Emerald is the historical default accent — losing it would silently rewrite
        // every user's colour on upgrade.
        var emerald = AccentTheme.Presets.First(p => p.Id == AccentTheme.DefaultId);
        Assert.Equal(Color.FromRgb(0x4A, 0xD6, 0x95), emerald.BaseColor);
    }

    [Fact]
    public void ResolveBase_UnknownIdFallsBackToDefault()
    {
        var color = AccentTheme.ResolveBase("does-not-exist", null);
        Assert.Equal(AccentTheme.Presets[0].BaseColor, color);
    }

    [Fact]
    public void ResolveBase_CustomIdWithMissingHex_FallsBackToDefault()
    {
        var color = AccentTheme.ResolveBase(AccentTheme.CustomId, null);
        Assert.Equal(AccentTheme.Presets[0].BaseColor, color);
    }

    [Fact]
    public void ResolveBase_CustomIdWithHex_ReturnsParsedColour()
    {
        var color = AccentTheme.ResolveBase(AccentTheme.CustomId, "#7AA2F7");
        Assert.Equal(Color.FromRgb(0x7A, 0xA2, 0xF7), color);
    }

    [Fact]
    public void ResolveBase_CustomIdWithoutHash_StillParses()
    {
        var color = AccentTheme.ResolveBase(AccentTheme.CustomId, "BD93F9");
        Assert.Equal(Color.FromRgb(0xBD, 0x93, 0xF9), color);
    }

    [Fact]
    public void TryParseHexColor_GarbageInput_ReturnsFalse()
    {
        Assert.False(AccentTheme.TryParseHexColor("not-a-color", out _));
        Assert.False(AccentTheme.TryParseHexColor("", out _));
        Assert.False(AccentTheme.TryParseHexColor(null, out _));
    }

    [Fact]
    public void ToHex_RoundTripsThroughParse()
    {
        var color = Color.FromRgb(0xCA, 0xFF, 0x33);
        var hex = AccentTheme.ToHex(color);
        Assert.Equal("#CAFF33", hex);
        Assert.True(AccentTheme.TryParseHexColor(hex, out var back));
        Assert.Equal(color, back);
    }

    [Fact]
    public void Derive_DarkBg_HoverIsBrighterThanBase()
    {
        var emerald = Color.FromRgb(0x4A, 0xD6, 0x95);
        var derived = AccentTheme.Derive(emerald, isDarkBg: true);

        // Base is passed through untouched on dark surfaces (no clamp fires).
        Assert.Equal(emerald, derived.Accent);
        // Hover has strictly higher luminance than pressed on the dark path.
        var (_, _, lHover) = AccentTheme.RgbToHsl(derived.Hover);
        var (_, _, lPressed) = AccentTheme.RgbToHsl(derived.Pressed);
        Assert.True(lHover > lPressed);
    }

    [Fact]
    public void Derive_LightBg_ClampsLuminanceForBrightBases()
    {
        // Praxvon Lime is very bright (L ~0.6 in HSL) — on a white bg the clamp must
        // pull it below the LightLuminanceCap so it actually reads.
        var lime = Color.FromRgb(0xCA, 0xFF, 0x33);
        var derived = AccentTheme.Derive(lime, isDarkBg: false);

        var (_, _, lAccent) = AccentTheme.RgbToHsl(derived.Accent);
        Assert.True(lAccent <= 0.42 + 0.001,
            $"expected luminance <= 0.42 on light bg after clamp, got {lAccent:F3}");
    }

    [Fact]
    public void Derive_LightBg_HoverIsDarkerThanBase()
    {
        // On light surfaces hover / pressed both go DOWN in luminance so the interaction
        // reads as "getting darker on hover", matching macOS / Material light-theme buttons.
        var sky = Color.FromRgb(0x7A, 0xA2, 0xF7);
        var derived = AccentTheme.Derive(sky, isDarkBg: false);

        var (_, _, lAccent) = AccentTheme.RgbToHsl(derived.Accent);
        var (_, _, lHover) = AccentTheme.RgbToHsl(derived.Hover);
        var (_, _, lPressed) = AccentTheme.RgbToHsl(derived.Pressed);
        Assert.True(lHover <= lAccent);
        Assert.True(lPressed <= lHover);
    }

    [Fact]
    public void Derive_GlowIsAccentAtTenPercentAlpha()
    {
        // Glow is used for soft focus rings around sliders; keeping it at alpha 0x1A
        // (~10 %) matches the existing palette convention so nothing looks louder or
        // quieter across themes.
        var derived = AccentTheme.Derive(Color.FromRgb(0x4A, 0xD6, 0x95), isDarkBg: true);
        Assert.Equal(0x1A, derived.Glow.A);
        Assert.Equal(derived.Accent.R, derived.Glow.R);
        Assert.Equal(derived.Accent.G, derived.Glow.G);
        Assert.Equal(derived.Accent.B, derived.Glow.B);
    }

    [Theory]
    [InlineData(0x00, 0x00, 0x00)] // pure black
    [InlineData(0xFF, 0xFF, 0xFF)] // pure white
    [InlineData(0x80, 0x80, 0x80)] // pure grey
    [InlineData(0x4A, 0xD6, 0x95)] // emerald
    [InlineData(0xCA, 0xFF, 0x33)] // lime
    [InlineData(0xBD, 0x93, 0xF9)] // violet
    public void RgbToHsl_ThenHslToRgb_RoundTripsWithinRoundingTolerance(byte r, byte g, byte b)
    {
        var original = Color.FromRgb(r, g, b);
        var (h, s, l) = AccentTheme.RgbToHsl(original);
        var back = AccentTheme.HslToRgb(h, s, l);
        // Allow +/-1 per channel from the double->byte rounding.
        Assert.InRange(back.R, (byte)Math.Max(0, r - 1), (byte)Math.Min(255, r + 1));
        Assert.InRange(back.G, (byte)Math.Max(0, g - 1), (byte)Math.Min(255, g + 1));
        Assert.InRange(back.B, (byte)Math.Max(0, b - 1), (byte)Math.Min(255, b + 1));
    }

    /// <summary>
    /// A light-theme accent must be tellable from the panel it is drawn on, and must not shout.
    ///
    /// Reported as "the colours are too bright in light mode, especially the light ones", with
    /// Praxvon's lime as the example. Two separate faults sat behind it and the measurement
    /// separated them: the accent kept FULL saturation because only lightness was ever capped,
    /// and its bar reached 1.5:1 against its own surface — a colour both loud and invisible, which
    /// sounds contradictory until you notice that saturation and luminance are different axes.
    ///
    /// The dark theme reached 14.8:1 by accident of the palette. Nothing made the light one do
    /// the same, so now something does.
    /// </summary>
    [Theory]
    [InlineData(0xCA, 0xFF, 0x33)] // Praxvon lime — the reported case
    [InlineData(0x4A, 0xD6, 0x95)] // emerald
    [InlineData(0xF5, 0xA6, 0x23)] // amber
    [InlineData(0x7A, 0xA2, 0xF7)] // sky
    [InlineData(0xFF, 0xFF, 0x00)] // the worst case: pure saturated yellow
    public void Derive_LightBg_AccentReadsAgainstItsOwnSurface(byte r, byte g, byte b)
    {
        var baseColor = Color.FromRgb(r, g, b);
        var accent = AccentTheme.Derive(baseColor, isDarkBg: false).Accent;
        var surface = AccentTheme.DeriveOsdSurfaces(baseColor, isDarkBg: false).SurfaceEnd;

        var ratio = ContrastInk.ContrastRatio(accent, surface);
        Assert.True(ratio >= 3.0 - 0.05,
            $"#{r:X2}{g:X2}{b:X2} gave a bar at {ratio:F2}:1 against its own panel");

        // Tolerance of 0.02, not 0.001. The accent is converted to RGB bytes and read back, and
        // that round trip moves saturation by more than a thousandth - the first version of this
        // assertion failed on a colour that had been damped exactly as intended. The claim worth
        // making is "damped", and lime arrives at 1.00, so the margin is not what is being tested.
        var (_, s, _) = AccentTheme.RgbToHsl(accent);
        Assert.True(s <= 0.80 + 0.02, $"light-theme saturation {s:F2} was not damped");
    }

    /// <summary>The dark theme is untouched by that: an accent that already reads is passed
    /// through at its own saturation, which is what keeps the OSD looking like itself.</summary>
    [Theory]
    [InlineData(0xCA, 0xFF, 0x33)]
    [InlineData(0x4A, 0xD6, 0x95)]
    public void Derive_DarkBg_LeavesTheAccentAlone(byte r, byte g, byte b)
    {
        var baseColor = Color.FromRgb(r, g, b);
        Assert.Equal(baseColor, AccentTheme.Derive(baseColor, isDarkBg: true).Accent);
    }

    [Fact]
    public void DeriveOsdSurfaces_DarkBg_KeepsSurfacesInDarkRange()
    {
        // Dark OSD surfaces must stay dark enough to read over exclusive-fullscreen
        // games. Cap at L <= 0.15 for the surface stops so a bright pick doesn't
        // wash out gameplay.
        foreach (var preset in AccentTheme.Presets)
        {
            var osd = AccentTheme.DeriveOsdSurfaces(preset.BaseColor, isDarkBg: true);
            var (_, _, lStart) = AccentTheme.RgbToHsl(osd.SurfaceStart);
            var (_, _, lEnd) = AccentTheme.RgbToHsl(osd.SurfaceEnd);
            Assert.True(lStart <= 0.15 + 0.001,
                $"{preset.Id} SurfaceStart luminance {lStart:F3} exceeded 0.15 on dark bg");
            Assert.True(lEnd <= 0.15 + 0.001,
                $"{preset.Id} SurfaceEnd luminance {lEnd:F3} exceeded 0.15 on dark bg");
        }
    }

    /// <summary>
    /// The accent moves the surface only at the extremes, and every built-in preset is inside.
    ///
    /// Both halves matter. Without the pull, choosing black on the light theme produced a
    /// near-white panel — the accent contributed hue and saturation, and black has neither, so
    /// the lightness stayed where the theme put it. Reported as "I give it black and it looks
    /// white", and it did.
    ///
    /// Without the band, the fix reached past the case it was for: at a band top of 0.75 the
    /// violet preset (L 0.776) started lightening the dark surface, changing a shipped look to
    /// serve a custom pick nobody had made. The test above caught that, which is why the band is
    /// set from the presets rather than chosen.
    /// </summary>
    [Fact]
    public void DeriveOsdSurfaces_FollowsAnExtremeAccentButLeavesPresetsAlone()
    {
        // An extreme is obeyed: black asks for a dark panel even on the light theme.
        var black = AccentTheme.DeriveOsdSurfaces(Color.FromRgb(0, 0, 0), isDarkBg: false);
        var (_, _, blackL) = AccentTheme.RgbToHsl(black.SurfaceEnd);
        Assert.True(blackL < 0.30, $"a black accent left the light surface at {blackL:F3}");

        // And symmetrically on the other theme.
        var white = AccentTheme.DeriveOsdSurfaces(Color.FromRgb(255, 255, 255), isDarkBg: true);
        var (_, _, whiteL) = AccentTheme.RgbToHsl(white.SurfaceEnd);
        Assert.True(whiteL > 0.70, $"a white accent left the dark surface at {whiteL:F3}");

        // While every preset produces exactly the theme's own surface, untouched.
        foreach (var preset in AccentTheme.Presets)
        {
            foreach (var isDark in new[] { true, false })
            {
                var osd = AccentTheme.DeriveOsdSurfaces(preset.BaseColor, isDark);
                var (_, _, l) = AccentTheme.RgbToHsl(osd.SurfaceEnd);
                // The theme's own surface, which moved when the tint was strengthened: the panel
                // was deviating from a neutral grey by about 20 channel units on either theme,
                // which is no tint at all. Colour needs room in the channels, and at L 0.07 or
                // 0.90 there is none - so the panel came in from the extreme rather than simply
                // taking more saturation, which would have changed nothing.
                var expected = isDark ? 0.12 : 0.84;

                Assert.True(Math.Abs(l - expected) < 0.005,
                    $"{preset.Id} on {(isDark ? "dark" : "light")} moved the surface to {l:F3}, expected {expected:F2}");
            }
        }
    }

    [Fact]
    public void DeriveOsdSurfaces_LightBg_KeepsSurfacesInLightRange()
    {
        foreach (var preset in AccentTheme.Presets)
        {
            var osd = AccentTheme.DeriveOsdSurfaces(preset.BaseColor, isDarkBg: false);
            var (_, _, lStart) = AccentTheme.RgbToHsl(osd.SurfaceStart);
            var (_, _, lEnd) = AccentTheme.RgbToHsl(osd.SurfaceEnd);
            Assert.True(lStart >= 0.80 - 0.001,
                $"{preset.Id} SurfaceStart luminance {lStart:F3} below 0.80 on light bg");
            Assert.True(lEnd >= 0.80 - 0.001,
                $"{preset.Id} SurfaceEnd luminance {lEnd:F3} below 0.80 on light bg");
        }
    }

    [Fact]
    public void DeriveOsdSurfaces_PreservesAccentHue()
    {
        // Sky (#7AA2F7, hue ~220) tinted for a dark card must still read as blue-ish —
        // i.e. the surface hue is close to the base hue, not shifted into a different
        // colour family by the desaturation clamp.
        var sky = Color.FromRgb(0x7A, 0xA2, 0xF7);
        var (baseH, _, _) = AccentTheme.RgbToHsl(sky);
        var osd = AccentTheme.DeriveOsdSurfaces(sky, isDarkBg: true);
        var (surfH, _, _) = AccentTheme.RgbToHsl(osd.SurfaceStart);
        Assert.InRange(surfH, baseH - 2, baseH + 2);
    }

    [Fact]
    public void HslToRgb_ZeroSaturation_IsAchromaticGrey()
    {
        // Regression: the achromatic short-circuit path was easy to skip and would
        // route through the hue-wedge math, producing colours that were technically
        // grey but not always exactly R==G==B.
        var grey = AccentTheme.HslToRgb(h: 180.0, s: 0.0, l: 0.5);
        Assert.Equal(grey.R, grey.G);
        Assert.Equal(grey.G, grey.B);
    }
}
