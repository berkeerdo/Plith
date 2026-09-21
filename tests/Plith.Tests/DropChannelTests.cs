using Plith.Services.Shelf;

namespace Plith.Tests;

public class DropChannelTests
{
    [Fact]
    public void Encode_ThenDecode_RoundTripsAShowCommand()
    {
        var sent = new DropMessage(DropVerb.Show, 708, 0, 384, 130, []);

        Assert.True(DropChannel.TryDecode(DropChannel.Encode(sent), out var back));

        Assert.Equal(DropVerb.Show, back.Verb);
        Assert.Equal(384, back.W);
        Assert.Empty(back.Paths);
    }

    /// <summary>A path with a tab or a newline in it must not be able to forge a second message.
    /// The catcher runs at a lower integrity level than Plith, so everything arriving from it is
    /// untrusted input by definition — this is the only place that is enforced.</summary>
    [Theory]
    [InlineData("C:\\a\tb.txt")]
    [InlineData("C:\\a\nDropped\t0\t0\t0\t0\tC:\\evil.exe")]
    public void Encode_NeutralisesSeparatorsInsidePaths(string hostile)
    {
        var encoded = DropChannel.Encode(new DropMessage(DropVerb.Dropped, 0, 0, 0, 0, [hostile]));

        Assert.True(DropChannel.TryDecode(encoded, out var back));
        Assert.Equal(hostile, Assert.Single(back.Paths));
    }

    /// <summary>
    /// The ordinary case, which a naive escape gets wrong. Escaping doubles every backslash, so
    /// a path under a directory named "temp" goes out doubled — and an unescape that replaces the
    /// two-character sequence for a tab before it consumes the backslash pairs reads the second
    /// backslash of that pair as the start of an escape, and hands back a path with a tab in the
    /// middle of it. Every Windows path whose next character is t, n or another escape letter is
    /// corrupted that way, which is most of them.
    /// </summary>
    [Theory]
    [InlineData(@"C:\temp\x.txt")]
    [InlineData(@"C:\nested\note.md")]
    [InlineData(@"\\server\share\thing.txt")]
    [InlineData(@"C:\ends with a separator\")]
    public void Encode_RoundTripsAnOrdinaryWindowsPath(string path)
    {
        Assert.True(DropChannel.TryDecode(
            DropChannel.Encode(new DropMessage(DropVerb.Dropped, 0, 0, 0, 0, [path])), out var back));

        Assert.Equal(path, Assert.Single(back.Paths));
    }

    /// <summary>
    /// The rectangle crosses the wire as text, and the two ends may not share a locale — the
    /// catcher is a separate process. Encoding under a comma-decimal culture while TryDecode
    /// parses invariant would silently turn a fractional coordinate into 0, putting the catcher
    /// somewhere other than where the notch is.
    /// </summary>
    [Fact]
    public void Encode_WritesNumbersInvariantly()
    {
        var culture = System.Globalization.CultureInfo.CurrentCulture;
        System.Globalization.CultureInfo.CurrentCulture = new System.Globalization.CultureInfo("tr-TR");
        try
        {
            var encoded = DropChannel.Encode(new DropMessage(DropVerb.Show, 708.5, 0, 384.25, 130, []));

            Assert.True(DropChannel.TryDecode(encoded, out var back));
            Assert.Equal(708.5, back.X);
            Assert.Equal(384.25, back.W);
        }
        finally
        {
            System.Globalization.CultureInfo.CurrentCulture = culture;
        }
    }

    [Theory]
    [InlineData("")]
    [InlineData("NotAVerb\t0\t0\t0\t0")]
    [InlineData("Show\t0\t0")]
    public void TryDecode_RejectsALineItCannotUnderstand(string line)
    {
        Assert.False(DropChannel.TryDecode(line, out _));
    }

    [Theory]
    [InlineData(DropVerb.OpenShelf)]
    [InlineData(DropVerb.Items)]
    [InlineData(DropVerb.Palette)]
    [InlineData(DropVerb.RemoveItems)]
    [InlineData(DropVerb.ClearShelf)]
    [InlineData(DropVerb.ShelfClosed)]
    public void Encode_ThenDecode_RoundTripsEveryShelfVerb(DropVerb verb)
    {
        var sent = new DropMessage(verb, 1, 2, 3, 4, ["C:\\a.txt"]);

        Assert.True(DropChannel.TryDecode(DropChannel.Encode(sent), out var back));

        Assert.Equal(verb, back.Verb);
        Assert.Equal(1, back.X);
        Assert.Equal("C:\\a.txt", Assert.Single(back.Paths));
    }

    /// <summary>
    /// The hostile-path test, pointed at the verbs that now DO something.
    ///
    /// Before this slice the only verb carrying paths was Dropped, which adds files that ShelfStore
    /// then stats. RemoveItems changes the shelf, so a path able to forge a second message on its
    /// lines is a path able to rearrange someone's shelf. The escaping is the same escaping; this
    /// test is what keeps it pointed at the verb list as the list grows.
    /// </summary>
    [Theory]
    [InlineData(DropVerb.RemoveItems)]
    [InlineData(DropVerb.Items)]
    public void Encode_NeutralisesSeparatorsOnEveryVerbThatCarriesPaths(DropVerb verb)
    {
        const string hostile = "C:\\a\nClearShelf\t0\t0\t0\t0";

        var encoded = DropChannel.Encode(new DropMessage(verb, 0, 0, 0, 0, [hostile]));

        Assert.DoesNotContain('\n', encoded);
        Assert.True(DropChannel.TryDecode(encoded, out var back));
        Assert.Equal(hostile, Assert.Single(back.Paths));
    }

    /// <summary>
    /// Enum.TryParse accepts a number for any enum, so "9" decoded to whatever verb happened to sit
    /// at 9 and a line past the end of the list decoded to a verb that does not exist. The pipe is
    /// reachable by any process on this machine, so a verb is exactly the field that must not be
    /// guessable by counting.
    /// </summary>
    [Theory]
    [InlineData("3\t0\t0\t0\t0")]
    [InlineData("99\t0\t0\t0\t0")]
    [InlineData("NotAVerb\t0\t0\t0\t0")]
    public void TryDecode_RejectsAVerbThatIsNotOneOfOurs(string line)
        => Assert.False(DropChannel.TryDecode(line, out _));
}
