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

    // And a saturation guardrail, which the cap above never had.
    //
    // Reported as "the colours are too bright in light mode, especially the light ones", and the
    // measurement agreed: lime came out at full saturation, S 1.00, sitting at L 0.42 on a pale
    // panel. Damping lightness alone leaves a colour just as loud - the surfaces have been damping
    // saturation since they were written, and the accent itself was the one thing that was not.
    private const double LightSaturationCap = 0.80;

    // How much a light-theme accent must stand out from the panel it is drawn on.
    //
    // 3:1 is WCAG's bar for a non-text shape, which is what a 6 DIP level bar is. Measured before
    // this existed: lime's bar against its own surface was 1.5:1 - the same colour family at the
    // same luminance, so a bar that was loud and invisible at once. The dark theme reached 14.8:1
    // by accident of the palette, and nothing made the light one do the same.
    private const double LightAccentOnSurface = 3.0;

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

        if (!isDarkBg)
        {
            if (l > LightLuminanceCap) l = LightLuminanceCap;
            if (s > LightSaturationCap) s = LightSaturationCap;

            // Then darken until the accent actually reads against the panel it will sit on.
            //
            // The surface is derived from the same base, so this asks the real question rather
            // than a proxy for it: not "is this colour dark enough in the abstract" but "can this
            // bar be told from the panel behind it". A fixed cap cannot answer that, because how
            // dark is dark enough depends on the hue - a saturated yellow at L 0.42 is far
            // brighter than a saturated blue at the same L, and only the measurement knows.
            var surface = DeriveOsdSurfaces(baseColor, isDarkBg: false).SurfaceEnd;
            while (l > 0.12 &&
                   ContrastInk.ContrastRatio(HslToRgb(h, s, l), surface) < LightAccentOnSurface)
            {
                l -= 0.02;
            }
        }

        var accent = HslToRgb(h, s, l);

        // Preferred direction, but only while there is room to move in it.
        //
        // These used to clamp instead: a dark theme brightened on hover, a light theme darkened,
        // and both saturated at the end of the scale. Pure black in the light theme therefore
        // produced hover and pressed identical to the accent - every accent-coloured control
        // went dead, responding to neither hover nor press. Pure white did the same on the dark
        // theme. Reported as "it breaks when I pick black", and it did.
        //
        // Clamping is the wrong answer at an extreme because the answer is not "as far as you
        // can go", it is "somewhere visibly different". When the preferred direction has no
        // headroom, the other one has all of it.
        var hover = HslToRgb(h, s, Nudge(l, 0.06, preferUp: isDarkBg));
        var pressed = HslToRgb(h, s, Nudge(l, isDarkBg ? 0.08 : 0.12, preferUp: false));
        var glow = Color.FromArgb(0x1A, accent.R, accent.G, accent.B);
        return new AccentDerived(accent, hover, pressed, glow);
    }

    /// <summary>
    /// Move a lightness by <paramref name="delta"/>, in the preferred direction when it fits and
    /// the opposite one when it does not.
    ///
    /// The point is that the result must DIFFER from where it started. A clamp satisfies the
    /// bounds and loses the difference, which is exactly what it must not do for a hover state.
    /// </summary>
    private static double Nudge(double l, double delta, bool preferUp)
    {
        if (preferUp && l + delta <= 1.0) return l + delta;
        if (!preferUp && l - delta >= 0.0) return l - delta;

        // No room the preferred way; the other way has it by definition, since delta is small.
        return preferUp
            ? System.Math.Max(0.0, l - delta)
            : System.Math.Min(1.0, l + delta);
    }

    /// <summary>
    /// Derives the tinted OSD card surfaces from a base accent so the whole overlay feels themed,
    /// not just the volume bar.
    ///
    /// The theme sets where this starts: dark surfaces around L 0.07-0.11 so the OSD stays
    /// readable over a full-screen game, light ones around 0.90-0.95 so they hold up on bright
    /// content. Saturation is damped either way, so a loud primary reads as a tint rather than as
    /// paint — the bar itself still sits at the full accent tone.
    ///
    /// What the accent ALSO does now is pull that lightness toward its own.
    ///
    /// It used to contribute only hue and saturation, which made one case absurd: choosing black
    /// on the light theme produced a near-white panel, because black has no hue to tint with and
    /// the lightness was fixed by the theme. Reported exactly that way — "I give it black and it
    /// looks white". The accent was a hue signal wearing the clothes of a colour picker.
    ///
    /// The pull only engages outside the band the theme is comfortable in, so every accent that
    /// looks right today is untouched: a lime, a pink, a blue all sit well inside it and produce
    /// precisely what they produced before. It is the far ends — the choices that currently read
    /// as broken — that move. Readability does not depend on where they land: the ink is computed
    /// from the surface by <see cref="ContrastInk"/>, and scripts/check-contrast.ps1 measures
    /// every accent against both themes.
    /// </summary>
    public static OsdSurfaceDerived DeriveOsdSurfaces(Color baseColor, bool isDarkBg)
    {
        var (h, s, l) = RgbToHsl(baseColor);

        // Stronger than it was, in both themes, because it was not carrying the accent at all.
        //
        // Measured before changing it: the panel deviated from a neutral grey of the same
        // lightness by about 20 channel units on either theme - invisible unless two of them are
        // side by side. Reported as "the colour I give does not show, it goes white", which on
        // the light theme is literally what a 1.3:1 difference from white looks like.
        //
        // Raising saturation alone would not have helped, and that is the part worth keeping:
        // colour needs room in the channels to exist, and at L 0.07 or L 0.90 there is none. The
        // panel had to come in from the extreme. Dark stops at 0.15, which is the limit a test
        // has enforced since long before this for readability over a full-screen game.
        double surfSat = System.Math.Min(s, isDarkBg ? 0.70 : 0.55);

        var startL = SurfaceLightness(l, isDarkBg, themeDefault: isDarkBg ? 0.15 : 0.88);
        var endL = SurfaceLightness(l, isDarkBg, themeDefault: isDarkBg ? 0.12 : 0.84);

        // Border, track and divider are offsets FROM the surface rather than absolute values, and
        // they move away from it — toward whichever side has the contrast. As absolutes they were
        // correct only while the surface stayed where the theme put it; a panel pulled dark on the
        // light theme would have kept a border lighter than nothing it sits on.
        var away = endL < 0.5 ? 1.0 : -1.0;

        return new OsdSurfaceDerived(
            SurfaceStart: HslToRgb(h, surfSat, startL),
            SurfaceEnd:   HslToRgb(h, surfSat, endL),
            Border:       HslToRgb(h, surfSat, Clamp01(endL + away * 0.15)),
            TrackBg:      HslToRgb(h, surfSat, Clamp01(endL + away * 0.21)),
            Divider:      HslToRgb(h, surfSat, Clamp01(endL + away * 0.13)));
    }

    /// <summary>
    /// Where a surface sits: the theme's own lightness, pulled toward the accent's when the accent
    /// is outside the band the theme is comfortable in.
    ///
    /// The band is deliberately wide. Inside it nothing happens at all, which is what keeps every
    /// accent that already looks right looking identical; outside it the pull ramps in smoothly,
    /// so there is no step where one hex behaves completely differently from the one next to it.
    ///
    /// The target is the accent's lightness eased off the very end — a surface at pure black or
    /// pure white is a hole or a glare rather than a panel, and the accent is still a tint even
    /// when it is being obeyed.
    /// </summary>
    private static double SurfaceLightness(double accentL, bool isDarkBg, double themeDefault)
    {
        // Dark themes are comfortable with any accent up to fairly bright; light themes with any
        // accent down to fairly dark. Beyond that the theme and the choice disagree, and the
        // choice is the one the user made.
        // Set from the built-in presets rather than by taste: the highest of them is violet at
        // L 0.776, and a preset is by definition a choice the product says looks right. A band
        // that catches one would change a shipped look to fix a custom pick nobody made.
        // Measured, not guessed - 0.75 caught violet, and the test that has asserted dark
        // surfaces stay dark since before any of this existed is what said so.
        const double DarkBandTop = 0.85;
        const double LightBandBottom = 0.25;

        double t;
        double target;

        if (isDarkBg)
        {
            if (accentL <= DarkBandTop) return themeDefault;
            t = (accentL - DarkBandTop) / (1.0 - DarkBandTop);
            target = accentL - 0.10;
        }
        else
        {
            if (accentL >= LightBandBottom) return themeDefault;
            t = (LightBandBottom - accentL) / LightBandBottom;
            target = accentL + 0.10;
        }

        return Clamp01(themeDefault + (target - themeDefault) * Clamp01(t));
    }

    private static double Clamp01(double v) => System.Math.Clamp(v, 0.0, 1.0);

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
