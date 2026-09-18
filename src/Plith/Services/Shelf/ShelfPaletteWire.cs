using System.Globalization;
using System.Windows.Media;

namespace Plith.Services.Shelf;

/// <param name="SurfaceStart">Top of the panel's gradient. Two colours rather than one because
/// OsdSurfaceBrush is a LinearGradientBrush: a single flat stand-in would be visibly not the
/// product's surface, which is the exact drift this type exists to prevent.</param>
/// <param name="IsDark">Carried rather than inferred from the ink. The catcher needs it for the
/// things a contrast ratio does not answer, such as which way a shadow falls.</param>
public readonly record struct ShelfPalette(
    Color SurfaceStart, Color SurfaceEnd, Color Ink, Color InkMuted, Color Track, Color Accent, bool IsDark);

/// <summary>
/// The theme, crossing a process boundary.
///
/// Plith derives its inks from the colour behind them through Services/ContrastInk.cs. A second
/// copy of that derivation in the catcher would drift, and the drift would show as the shelf not
/// looking like the product it belongs to. So the catcher is told the answer rather than given
/// the method.
///
/// It rides in the path list of a Palette message, which costs no new format: the list is
/// already escaped, already ordered, and already carried by every message.
/// </summary>
public static class ShelfPaletteWire
{
    /// <summary>Six colours and a flag. The ORDER is the contract; a named format would mean a
    /// parser on the far side and a second thing to keep in step.</summary>
    public const int FieldCount = 7;

    public static IReadOnlyList<string> ToPaths(ShelfPalette p) =>
    [
        Hex(p.SurfaceStart), Hex(p.SurfaceEnd), Hex(p.Ink), Hex(p.InkMuted), Hex(p.Track), Hex(p.Accent),
        p.IsDark ? "1" : "0",
    ];

    public static bool TryFromPaths(IReadOnlyList<string> paths, out ShelfPalette palette)
    {
        palette = default;
        if (paths.Count != FieldCount) return false;

        var colors = new Color[6];
        for (var i = 0; i < 6; i++)
        {
            if (!TryParseHex(paths[i], out colors[i])) return false;
        }

        if (paths[6] is not ("0" or "1")) return false;

        palette = new ShelfPalette(colors[0], colors[1], colors[2], colors[3], colors[4], colors[5],
                                   IsDark: paths[6] == "1");
        return true;
    }

    private static string Hex(Color c) =>
        string.Create(CultureInfo.InvariantCulture, $"#{c.A:X2}{c.R:X2}{c.G:X2}{c.B:X2}");

    /// <summary>
    /// Parsed by hand rather than through ColorConverter, because ColorConverter accepts named
    /// colours and malformed input by throwing, and this input arrives over a pipe any process
    /// can write. A parser that throws on hostile input is a parser that takes the catcher down.
    ///
    /// The null check on <paramref name="value"/> is not academic despite the non-nullable
    /// parameter type: the values in a decoded DropMessage arrive from another process, over a
    /// wire that carries text, and the C# type system describes what this method promises to
    /// callers on this side of that boundary, not what the boundary itself can guarantee. A null
    /// element must fail this parse the same as any other malformed value, not throw past it.
    /// </summary>
    private static bool TryParseHex(string? value, out Color color)
    {
        color = default;
        if (value is null || value.Length != 9 || value[0] != '#') return false;

        var channels = new byte[4];
        for (var i = 0; i < 4; i++)
        {
            if (!byte.TryParse(value.AsSpan(1 + i * 2, 2), NumberStyles.HexNumber,
                               CultureInfo.InvariantCulture, out channels[i]))
                return false;
        }

        color = Color.FromArgb(channels[0], channels[1], channels[2], channels[3]);
        return true;
    }
}
