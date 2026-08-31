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
