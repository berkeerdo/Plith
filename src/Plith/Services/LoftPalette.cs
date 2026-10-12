using System.Windows.Media;

namespace Plith.Services;

/// <summary>One mode's floor, from praxvon-ui `tokens/themes/praxvon.json`.</summary>
public sealed record LoftFloor(
    Color Background,
    Color Surface,
    Color Foreground,
    Color TextSecondary,
    Color TextTertiary,
    Color Border,
    Color BorderStrong);

/// <summary>The four roles one accent seed derives into, per ADR 0014.</summary>
public sealed record LoftAccentRoles(Color Fill, Color OnFill, Color Mark, Color Text);

/// <summary>
/// Loft's fixed floors. The surface no longer moves with the user's accent, which is the whole
/// change: ink against a known floor is a constant, and the five palette keys that existed to
/// compute ink against an unknown one are gone.
/// </summary>
public static class LoftPalette
{
    public static readonly LoftFloor DarkFloor = new(
        Background:    Rgb(0x0B, 0x0B, 0x0A),
        Surface:       Rgb(0x15, 0x14, 0x12),
        Foreground:    Rgb(0xEC, 0xE9, 0xE2),
        TextSecondary: Rgb(0xA3, 0x9E, 0x94),
        TextTertiary:  Rgb(0x7A, 0x75, 0x6C),
        Border:        Rgb(0x2B, 0x28, 0x24),
        BorderStrong:  Rgb(0x7A, 0x75, 0x6C));

    public static readonly LoftFloor LightFloor = new(
        Background:    Rgb(0xE7, 0xE6, 0xE0),
        Surface:       Rgb(0xF3, 0xF2, 0xED),
        Foreground:    Rgb(0x16, 0x16, 0x16),
        TextSecondary: Rgb(0x55, 0x54, 0x4F),
        TextTertiary:  Rgb(0x77, 0x75, 0x6D),
        Border:        Rgb(0xD0, 0xCE, 0xC6),
        BorderStrong:  Rgb(0x77, 0x75, 0x6D));

    public static LoftFloor For(bool isDark) => isDark ? DarkFloor : LightFloor;

    private static Color Rgb(byte r, byte g, byte b) => Color.FromRgb(r, g, b);
}
