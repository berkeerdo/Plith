using System.Windows.Media;

namespace Plith.Services;

/// <summary>Ink for one surface: the body text colour and the quieter one under it.</summary>
public readonly record struct InkPair(Color Ink, Color Muted);

/// <summary>
/// Choosing text colour from the colour behind it.
///
/// Every place this replaces had the same shape: a foreground fixed in a palette or hard-coded in
/// a style, sitting on a background the user chooses. That works until the user chooses a colour
/// from the other end of the scale. Reported as a pink accent in light mode with white text on
/// it — the notch surface is tinted by the accent and goes pale, while NotchInk is a near-white
/// constant picked on the assumption that the notch is always a near-black cutout.
///
/// So the ink is computed instead of declared. WCAG's relative luminance and contrast ratio,
/// which is the same arithmetic a browser's accessibility tools use, and the same one that says
/// white on pale pink is about 1.7:1 — under a third of the 4.5:1 a body of text needs.
/// </summary>
public static class ContrastInk
{
    /// <summary>Near-black rather than pure black: on a bright surface pure black is harsher than
    /// it needs to be, and this still clears 4.5:1 anywhere #000 does except within a hair of the
    /// crossover.</summary>
    private static readonly Color Dark = Color.FromRgb(0x14, 0x17, 0x1C);

    private static readonly Color Light = Color.FromRgb(0xF2, 0xF5, 0xF8);

    /// <summary>
    /// WCAG relative luminance, 0 for black and 1 for white.
    ///
    /// Not the L of HSL, and the difference is the whole point: HSL lightness treats pure blue
    /// and pure yellow as equally light, while the eye does not. Blue at L=0.5 is dark enough to
    /// carry white text and yellow at L=0.5 is not, and only this formula knows that.
    /// </summary>
    public static double RelativeLuminance(Color c)
    {
        static double Channel(byte v)
        {
            var s = v / 255.0;
            return s <= 0.03928 ? s / 12.92 : Math.Pow((s + 0.055) / 1.055, 2.4);
        }

        return 0.2126 * Channel(c.R) + 0.7152 * Channel(c.G) + 0.0722 * Channel(c.B);
    }

    /// <summary>WCAG contrast ratio between two colours, 1.0 (identical) to 21.0 (black on
    /// white). 4.5 is the threshold for body text, 3.0 for large text and shapes.</summary>
    public static double ContrastRatio(Color a, Color b)
    {
        var la = RelativeLuminance(a);
        var lb = RelativeLuminance(b);
        var (hi, lo) = la >= lb ? (la, lb) : (lb, la);
        return (hi + 0.05) / (lo + 0.05);
    }

    /// <summary>
    /// The ink for a surface: whichever of the two extremes reads better on it.
    ///
    /// A straight comparison rather than a luminance threshold, because a threshold has to be
    /// guessed and this does not: ask both candidates what contrast they achieve and keep the
    /// winner. At the crossover the two are equal by definition, so there is no band where the
    /// answer is arbitrary.
    /// </summary>
    public static Color On(Color surface)
    {
        var near = ContrastRatio(surface, Dark) >= ContrastRatio(surface, Light) ? Dark : Light;
        if (ContrastRatio(surface, near) >= 4.5) return near;

        // The near-extremes are softened on purpose - pure black on a bright surface is harsher
        // than it needs to be - but softened is not worth an unreadable label. A mid-tone surface
        // is where that bill comes due: a slate accent measured 4.3:1 against the near-black and
        // 4.6:1 against true black, so the softening was the whole difference between passing and
        // failing. Fall the rest of the way only when it actually buys the threshold.
        var pure = ContrastRatio(surface, Colors.Black) >= ContrastRatio(surface, Colors.White)
            ? Colors.Black
            : Colors.White;

        return ContrastRatio(surface, pure) > ContrastRatio(surface, near) ? pure : near;
    }

    /// <summary>
    /// Body ink and a quieter secondary, both readable on the same surface.
    ///
    /// The muted one is mixed toward the surface rather than picked from a palette, so it stays
    /// quieter than the body text on any background instead of only on the one it was chosen
    /// against. It is held at 4.5:1 — the muted ink carries device names and dates, which is body
    /// text however small it is set.
    /// </summary>
    public static InkPair PairOn(Color surface)
    {
        var ink = On(surface);

        // Walk toward the surface while the result still clears the threshold, and keep the last
        // value that did. Stepping rather than solving because the relationship is not linear in
        // sRGB, and forty steps is exact enough for a colour.
        var muted = ink;
        for (var i = 1; i <= 40; i++)
        {
            var candidate = Mix(ink, surface, i / 40.0);
            if (ContrastRatio(surface, candidate) < 4.5) break;
            muted = candidate;
        }

        return new InkPair(ink, muted);
    }

    /// <summary>
    /// A fill that sits on a surface and must be distinguishable from it — a slider's groove, a
    /// tile's face. Not text, so 3:1 rather than 4.5:1, and it moves away from the surface rather
    /// than to an extreme, which is what keeps it reading as the same material.
    /// </summary>
    public static Color TrackOn(Color surface)
    {
        var toward = On(surface);

        var track = surface;
        for (var i = 1; i <= 40; i++)
        {
            track = Mix(surface, toward, i / 40.0);
            if (ContrastRatio(surface, track) >= 3.0) break;
        }

        return track;
    }

    private static Color Mix(Color from, Color to, double t)
    {
        static byte Lerp(byte a, byte b, double t) => (byte)Math.Round(a + (b - a) * t);
        return Color.FromRgb(Lerp(from.R, to.R, t), Lerp(from.G, to.G, t), Lerp(from.B, to.B, t));
    }
}
