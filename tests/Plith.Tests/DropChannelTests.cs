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
}
