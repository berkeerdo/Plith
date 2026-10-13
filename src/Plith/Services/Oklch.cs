using System.Windows.Media;

namespace Plith.Services;

/// <summary>
/// OKLab/OKLCH conversion and a lightness walk, ported from praxvon-ui's
/// `packages/cli/src/accent.ts` so the two derive the same roles from the same seed.
///
/// Why not the HSL already in <see cref="AccentTheme"/>: HSL lightness is not perceptual. A walk
/// in it changes apparent chroma as it moves, and two accents at the same HSL lightness read as
/// different brightnesses, so a ratio held in HSL holds by luck rather than by construction.
/// </summary>
public static class Oklch
{
    private const double Epsilon = 1e-9;

    public static (double L, double C, double H) FromColor(Color c)
    {
        var r = ToLinear(c.R / 255.0);
        var g = ToLinear(c.G / 255.0);
        var b = ToLinear(c.B / 255.0);

        var l = Math.Cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
        var m = Math.Cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
        var s = Math.Cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);

        var okL = 0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s;
        var okA = 1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s;
        var okB = 0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s;

        var chroma = Math.Sqrt(okA * okA + okB * okB);
        var hue = chroma < Epsilon ? 0.0 : Math.Atan2(okB, okA) * 180.0 / Math.PI;
        if (hue < 0) hue += 360.0;

        return (okL, chroma, hue);
    }

    public static Color ToColor(double okL, double chroma, double hue)
    {
        var rad = hue * Math.PI / 180.0;
        var okA = chroma * Math.Cos(rad);
        var okB = chroma * Math.Sin(rad);

        var l = okL + 0.3963377774 * okA + 0.2158037573 * okB;
        var m = okL - 0.1055613458 * okA - 0.0638541728 * okB;
        var s = okL - 0.0894841775 * okA - 1.2914855480 * okB;

        l = l * l * l; m = m * m * m; s = s * s * s;

        var r = +4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s;
        var g = -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s;
        var b = -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s;

        return Color.FromRgb(ToByte(r), ToByte(g), ToByte(b));
    }

    /// <summary>
    /// Step the seed's lightness in <paramref name="direction"/> until
    /// <paramref name="passes"/> accepts the result. Hue and chroma are held, so the walk changes
    /// how light a colour is and not which colour it is.
    ///
    /// Returns the endpoint tone when nothing passes, rather than the seed: a caller asking for a
    /// ratio gets the closest this hue can come to it, and the decision about what to do with a
    /// colour that cannot reach its ratio belongs to the caller, not to the walk.
    /// </summary>
    public static Color Walk(Color seed, Func<Color, bool> passes, int direction)
    {
        ArgumentNullException.ThrowIfNull(passes);
        if (passes(seed)) return seed;

        var (l, c, h) = FromColor(seed);
        const double Step = 0.01;

        var endpoint = direction > 0 ? 1.0 : 0.0;
        var last = seed;
        for (var i = 1; i <= 100; i++)
        {
            var next = l + direction * Step * i;

            // Clamp the last step to the endpoint rather than breaking short of it. Breaking
            // stops at 0.99, so white and black are never tried, and the walk can then return a
            // tone that fails while a passing tone existed one step further on.
            var atEnd = direction > 0 ? next >= endpoint : next <= endpoint;
            if (atEnd) next = endpoint;

            last = ToColor(next, c, h);
            if (passes(last)) return last;
            if (atEnd) break;
        }
        return last;
    }

    private static double ToLinear(double channel) =>
        channel <= 0.04045 ? channel / 12.92 : Math.Pow((channel + 0.055) / 1.055, 2.4);

    private static byte ToByte(double linear)
    {
        var srgb = linear <= 0.0031308 ? linear * 12.92 : 1.055 * Math.Pow(linear, 1.0 / 2.4) - 0.055;
        return (byte)Math.Clamp(Math.Round(srgb * 255.0), 0, 255);
    }
}
