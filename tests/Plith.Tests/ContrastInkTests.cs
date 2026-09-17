using System.Windows.Media;
using Plith.Services;
using Xunit;

namespace Plith.Tests;

/// <summary>
/// The arithmetic behind "which text colour goes on this background".
///
/// Written against the failure that produced it: a pink accent on the light theme gave a pale
/// panel carrying near-white text, measured afterwards at 1.2:1 where body text needs 4.5:1. The
/// interesting cases are not the obvious ends — black on white is nobody's bug — but the middle,
/// where a wrong implementation still looks plausible.
/// </summary>
public class ContrastInkTests
{
    private static Color Hex(string hex) => (Color)ColorConverter.ConvertFromString(hex)!;

    [Fact]
    public void ContrastRatio_IsTwentyOneForBlackOnWhite()
        => Assert.Equal(21.0, ContrastInk.ContrastRatio(Colors.Black, Colors.White), precision: 1);

    [Fact]
    public void ContrastRatio_IsOneForAColourAgainstItself()
        => Assert.Equal(1.0, ContrastInk.ContrastRatio(Hex("#3D7A22"), Hex("#3D7A22")), precision: 3);

    [Fact]
    public void ContrastRatio_DoesNotCareWhichArgumentIsWhich()
        => Assert.Equal(
            ContrastInk.ContrastRatio(Hex("#EC4899"), Colors.White),
            ContrastInk.ContrastRatio(Colors.White, Hex("#EC4899")), precision: 6);

    /// <summary>
    /// Luminance is perceptual, not HSL lightness — the distinction the whole thing rests on.
    ///
    /// Pure blue and pure yellow have identical HSL lightness. An implementation using L would
    /// give them the same ink and be wrong about one of them: yellow needs dark text and blue
    /// needs light. This is the test that fails if anyone "simplifies" it back to HSL.
    /// </summary>
    [Fact]
    public void RelativeLuminance_SeparatesBlueFromYellow()
    {
        var blue = ContrastInk.RelativeLuminance(Colors.Blue);
        var yellow = ContrastInk.RelativeLuminance(Colors.Yellow);

        Assert.True(yellow > blue * 5,
            $"yellow {yellow:F3} should be far brighter than blue {blue:F3}; HSL says they are equal");
    }

    [Theory]
    [InlineData("#FFFFFF")]
    [InlineData("#FACC15")]   // yellow
    [InlineData("#A3E635")]   // the user's lime
    [InlineData("#EEDDE5")]   // the pale pink surface from the bug report
    public void On_PutsDarkInkOnBrightSurfaces(string surface)
        => Assert.True(ContrastInk.RelativeLuminance(ContrastInk.On(Hex(surface))) < 0.2);

    [Theory]
    [InlineData("#000000")]
    [InlineData("#1E3A8A")]   // deep blue
    [InlineData("#0A0D12")]   // the media page's scrim
    public void On_PutsLightInkOnDarkSurfaces(string surface)
        => Assert.True(ContrastInk.RelativeLuminance(ContrastInk.On(Hex(surface))) > 0.7);

    /// <summary>
    /// The case the softened extremes got wrong.
    ///
    /// A mid-tone surface is the worst one: neither near-black nor near-white has much room. A
    /// slate accent measured 4.3:1 against the softened near-black and 4.6:1 against true black,
    /// so the softening — which exists to keep bright surfaces from being harsh — was the entire
    /// difference between passing and failing. On() falls the rest of the way only there.
    /// </summary>
    [Theory]
    [InlineData("#64748B")]
    [InlineData("#808080")]
    [InlineData("#767676")]
    public void On_ClearsTheTextThresholdEvenOnMidTones(string surface)
    {
        var s = Hex(surface);
        Assert.True(ContrastInk.ContrastRatio(s, ContrastInk.On(s)) >= 4.5,
            $"{surface} got {ContrastInk.ContrastRatio(s, ContrastInk.On(s)):F2}:1");
    }

    /// <summary>The muted ink is quieter than the body ink but still readable — a secondary line
    /// is still a line someone reads.</summary>
    [Theory]
    [InlineData("#141C08")]
    [InlineData("#E8EEDD")]
    [InlineData("#EEDDE5")]
    [InlineData("#64748B")]
    public void PairOn_KeepsTheMutedInkReadableAndQuieter(string surface)
    {
        var s = Hex(surface);
        var pair = ContrastInk.PairOn(s);

        Assert.True(ContrastInk.ContrastRatio(s, pair.Muted) >= 4.5,
            $"muted on {surface} was {ContrastInk.ContrastRatio(s, pair.Muted):F2}:1");
        Assert.True(ContrastInk.ContrastRatio(s, pair.Muted) <= ContrastInk.ContrastRatio(s, pair.Ink),
            "the muted ink must not end up louder than the body ink");
    }

    /// <summary>A groove or a track is read as a shape, so 3:1 — and it must move away from its
    /// surface rather than to an extreme, or it stops looking like the same material.</summary>
    [Theory]
    [InlineData("#141C08")]
    [InlineData("#E8EEDD")]
    [InlineData("#1E3A8A")]
    [InlineData("#A3E635")]
    public void TrackOn_SeparatesFromItsSurfaceWithoutGoingToAnExtreme(string surface)
    {
        var s = Hex(surface);
        var track = ContrastInk.TrackOn(s);

        Assert.True(ContrastInk.ContrastRatio(s, track) >= 3.0,
            $"track on {surface} was {ContrastInk.ContrastRatio(s, track):F2}:1");

        var toExtreme = ContrastInk.ContrastRatio(s, ContrastInk.On(s));
        Assert.True(ContrastInk.ContrastRatio(s, track) < toExtreme,
            "a track that reaches the ink's contrast has stopped being a recess and become text");
    }

    /// <summary>
    /// The reported bug, as a test.
    ///
    /// The light theme's notch surface for a pink accent, and the near-white constant that used to
    /// be painted on it. The old pairing has to fail and the computed one has to pass — asserting
    /// only the second would let a change that quietly reverts the first go unnoticed.
    /// </summary>
    [Fact]
    public void TheReportedCase_FailsAsDeclaredAndPassesAsComputed()
    {
        var palePink = Hex("#EEDDE5");
        var oldConstant = Hex("#F2F5F8");

        Assert.True(ContrastInk.ContrastRatio(palePink, oldConstant) < 2.0,
            "the reported pairing should be far below any threshold");
        Assert.True(ContrastInk.ContrastRatio(palePink, ContrastInk.On(palePink)) >= 4.5);
    }
}
