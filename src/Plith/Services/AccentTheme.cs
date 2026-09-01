using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using System.Windows.Media;

namespace Plith.Services;

/// <summary>
/// Named accent preset shown in the Settings picker. <see cref="Id"/> is what gets
/// persisted; <see cref="BaseColor"/> is the raw dark-bg tone from which hover /
/// pressed / glow variants are derived at apply time.
/// </summary>
public sealed record AccentPreset(string Id, string DisplayName, Color BaseColor);

/// <summary>Bundle of brushes computed from a single base accent, one per role.
/// The names map 1:1 to the palette dictionary keys the theme service overrides.</summary>
public sealed record AccentDerived(Color Accent, Color Hover, Color Pressed, Color Glow);

/// <summary>Surface tones derived from a base accent so the entire OSD card can be
/// tinted from the picked colour instead of only its bar. The alpha channel is
/// applied downstream by the theme service — this record carries opaque colours,
/// and ThemeService wraps them with the right F0 / 40 alpha to match the existing
/// gradient / track semi-transparency.</summary>
public sealed record OsdSurfaceDerived(
    Color SurfaceStart,
    Color SurfaceEnd,
    Color Border,
    Color TrackBg,
    Color Divider);

/// <summary>
/// Palette-independent accent registry and derivation math. The Settings picker
/// stores an id ("emerald", "lime", ..., or "custom" plus a hex string) and the
/// ThemeService calls <see cref="Derive"/> per theme swap to produce the four
/// derived tones that override the palette's Accent* brushes.
///
/// Hover / pressed variants are computed in HSL because linear-RGB shifts crush
/// saturation on dark accents and blow it out on light ones; HSL keeps the hue
/// stable while nudging luminance in the direction that matches the surface.
/// </summary>
public static class AccentTheme
{
    public const string DefaultId = "emerald";
    public const string CustomId = "custom";

    // Curated presets tuned to look decent on both dark and light surfaces.
    // The dark-bg palette uses the base colour as-is; the light-bg one clamps
    // luminance to <= LightLuminanceCap so lime / peach don't bleach out on white.
    public static readonly IReadOnlyList<AccentPreset> Presets = new List<AccentPreset>
    {
        new(DefaultId,   "Emerald",      Color.FromRgb(0x4A, 0xD6, 0x95)),
        new("lime",      "Praxvon Lime", Color.FromRgb(0xCA, 0xFF, 0x33)),
        new("sky",       "Sky",          Color.FromRgb(0x7A, 0xA2, 0xF7)),
        new("frost",     "Frost",        Color.FromRgb(0x88, 0xC0, 0xD0)),
        new("violet",    "Violet",       Color.FromRgb(0xBD, 0x93, 0xF9)),
        new("peach",     "Peach",        Color.FromRgb(0xFA, 0xB3, 0x87)),
        new("amber",     "Amber",        Color.FromRgb(0xF5, 0xA6, 0x23)),
        new("rose",      "Rose",         Color.FromRgb(0xF4, 0x72, 0xB6)),
    };

    // Contrast guardrail for light-bg accents: any base with HSL luminance above
    // this gets pulled down before deriving hover / pressed. Chosen so #CAFF33
    // (Praxvon Lime, native L ~0.6) still reads on #FFFFFF cards.
    private const double LightLuminanceCap = 0.42;

    /// <summary>
    /// Returns the base <see cref="Color"/> that <paramref name="id"/> refers to.
    /// For <see cref="CustomId"/>, <paramref name="customHex"/> is parsed; when it
    /// is missing or unparseable we fall through to the default preset so the app
    /// never renders with a null accent.
    /// </summary>
    public static Color ResolveBase(string? id, string? customHex)
    {
        if (string.Equals(id, CustomId, System.StringComparison.OrdinalIgnoreCase))
            return ParseHexColor(customHex, Presets[0].BaseColor);

        var preset = Presets.FirstOrDefault(p =>
            string.Equals(p.Id, id, System.StringComparison.OrdinalIgnoreCase));
        return preset?.BaseColor ?? Presets[0].BaseColor;
    }

    /// <summary>
    /// Produces the four derived brushes from a base colour, tuned for the current
    /// surface. On dark surfaces hover brightens and pressed darkens; on light
    /// surfaces both go darker (matching WPF Material / macOS button conventions)
    /// and the base itself gets luminance-clamped so it stays legible on white.
    /// Glow keeps the accent hue at 10 % alpha for slider tracks / focus rings.
    /// </summary>
    public static AccentDerived Derive(Color baseColor, bool isDarkBg)
    {
        var (h, s, l) = RgbToHsl(baseColor);
        if (!isDarkBg && l > LightLuminanceCap) l = LightLuminanceCap;

        var accent = HslToRgb(h, s, l);
        var hover = isDarkBg
            ? HslToRgb(h, s, System.Math.Min(1.0, l + 0.06))
            : HslToRgb(h, s, System.Math.Max(0.0, l - 0.06));
        var pressed = isDarkBg
            ? HslToRgb(h, s, System.Math.Max(0.0, l - 0.08))
            : HslToRgb(h, s, System.Math.Max(0.0, l - 0.12));
        var glow = Color.FromArgb(0x1A, accent.R, accent.G, accent.B);
        return new AccentDerived(accent, hover, pressed, glow);
    }

    /// <summary>
    /// Derives the tinted OSD card surfaces from a base accent so the whole overlay
    /// feels themed, not just the volume bar. Dark surfaces sit at L≈0.07-0.11 so
    /// the OSD stays readable over exclusive-fullscreen games; light surfaces sit
    /// at L≈0.90-0.94 so they stay legible on bright content. Saturation is damped
    /// so loud primaries (lime, magenta) don't turn the whole card into a beacon —
    /// the bar itself still sits at the full accent tone for contrast.
    /// </summary>
    public static OsdSurfaceDerived DeriveOsdSurfaces(Color baseColor, bool isDarkBg)
    {
        var (h, s, _) = RgbToHsl(baseColor);

        if (isDarkBg)
        {
            // Darker surfaces tolerate more saturation than lighter ones without
            // shouting; still cap so a highly saturated pick reads as a tint, not paint.
            double surfSat = System.Math.Min(s, 0.55);
            return new OsdSurfaceDerived(
                SurfaceStart: HslToRgb(h, surfSat, 0.11),
                SurfaceEnd:   HslToRgb(h, surfSat, 0.07),
                Border:       HslToRgb(h, surfSat, 0.22),
                TrackBg:      HslToRgb(h, surfSat, 0.28),
                Divider:      HslToRgb(h, surfSat, 0.20));
        }
        // Light surfaces bleach out fast — hold saturation lower so the tint stays
        // "a note of colour" instead of turning into pastel highlighter.
        double lightSat = System.Math.Min(s, 0.35);
        return new OsdSurfaceDerived(
            SurfaceStart: HslToRgb(h, lightSat, 0.95),
            SurfaceEnd:   HslToRgb(h, lightSat, 0.90),
            Border:       HslToRgb(h, lightSat, 0.78),
            TrackBg:      HslToRgb(h, lightSat, 0.84),
            Divider:      HslToRgb(h, lightSat, 0.82));
    }

    /// <summary>Parses "#RRGGBB" / "RRGGBB" / "#AARRGGBB". Returns <paramref name="fallback"/>
    /// on null / empty / malformed input so the caller never crashes on a corrupt config.</summary>
    public static Color ParseHexColor(string? hex, Color fallback) =>
        TryParseHexColor(hex, out var c) ? c : fallback;

    /// <summary>Bool-returning variant for callers that need to distinguish "user
    /// typed garbage" from "the fallback colour happens to equal a valid parse". Used
    /// by the Settings hex box to reject invalid input mid-typing.</summary>
    public static bool TryParseHexColor(string? hex, out Color color)
    {
        color = default;
        if (string.IsNullOrWhiteSpace(hex)) return false;
        var s = hex.Trim();
        if (!s.StartsWith('#')) s = "#" + s;
        try
        {
            var obj = ColorConverter.ConvertFromString(s);
            if (obj is Color c) { color = c; return true; }
            return false;
        }
        catch { return false; }
    }

    public static string ToHex(Color c) =>
        string.Create(CultureInfo.InvariantCulture, $"#{c.R:X2}{c.G:X2}{c.B:X2}");

    // Standard RGB<->HSL conversions. h in [0, 360), s and l in [0, 1].
    // Made public so the Settings custom-colour popup can round-trip Color <-> HSL for
    // its sliders; internal derivation still goes through Derive above.
    public static (double h, double s, double l) RgbToHsl(Color c)
    {
        double r = c.R / 255.0, g = c.G / 255.0, b = c.B / 255.0;
        double max = System.Math.Max(r, System.Math.Max(g, b));
        double min = System.Math.Min(r, System.Math.Min(g, b));
        double l = (max + min) / 2.0;
        double h = 0.0, s = 0.0;
        if (max != min)
        {
            double d = max - min;
            s = l > 0.5 ? d / (2.0 - max - min) : d / (max + min);
            if (max == r) h = (g - b) / d + (g < b ? 6.0 : 0.0);
            else if (max == g) h = (b - r) / d + 2.0;
            else h = (r - g) / d + 4.0;
            h *= 60.0;
        }
        return (h, s, l);
    }

    public static Color HslToRgb(double h, double s, double l)
    {
        // Handle achromatic short-circuit — HslToRgb of any h with s == 0 must
        // produce a pure grey, and the general path already does that, but the
        // explicit early-out keeps intent obvious.
        if (s <= 0.0)
        {
            byte v = (byte)System.Math.Round(System.Math.Clamp(l, 0.0, 1.0) * 255.0);
            return Color.FromRgb(v, v, v);
        }
        double c = (1.0 - System.Math.Abs(2.0 * l - 1.0)) * s;
        double hh = ((h % 360.0) + 360.0) % 360.0 / 60.0;
        double x = c * (1.0 - System.Math.Abs(hh % 2.0 - 1.0));
        double r1 = 0.0, g1 = 0.0, b1 = 0.0;
        if (hh < 1.0) { r1 = c; g1 = x; }
        else if (hh < 2.0) { r1 = x; g1 = c; }
        else if (hh < 3.0) { g1 = c; b1 = x; }
        else if (hh < 4.0) { g1 = x; b1 = c; }
        else if (hh < 5.0) { r1 = x; b1 = c; }
        else { r1 = c; b1 = x; }
        double m = l - c / 2.0;
        return Color.FromRgb(
            (byte)System.Math.Round(System.Math.Clamp(r1 + m, 0.0, 1.0) * 255.0),
            (byte)System.Math.Round(System.Math.Clamp(g1 + m, 0.0, 1.0) * 255.0),
            (byte)System.Math.Round(System.Math.Clamp(b1 + m, 0.0, 1.0) * 255.0));
    }
}
