using System.Windows.Media;
using Plith.Services.Shelf;

namespace Plith.Tests;

public sealed class ShelfPaletteWireTests
{
    private static ShelfPalette Sample() => new(
        Color.FromArgb(0xFF, 0x1A, 0x20, 0x28),
        Color.FromArgb(0xFF, 0x12, 0x16, 0x1C),
        Color.FromRgb(0xF2, 0xF5, 0xF8),
        Color.FromRgb(0x9A, 0xA6, 0xB2),
        Color.FromRgb(0x2A, 0x32, 0x3C),
        Color.FromRgb(0xA3, 0xE6, 0x35),
        IsDark: true);

    [Fact]
    public void ToPaths_ThenBack_RoundTripsEveryChannel()
    {
        var sent = Sample();

        Assert.True(ShelfPaletteWire.TryFromPaths(ShelfPaletteWire.ToPaths(sent), out var back));

        Assert.Equal(sent, back);
    }

    /// <summary>Seven values, and the order IS the contract. A short or long payload is a
    /// version mismatch between the two executables, and the catcher must fall back to its own
    /// colours rather than paint with whatever it managed to parse.</summary>
    [Theory]
    [InlineData(6)]
    [InlineData(8)]
    public void TryFromPaths_RefusesAPayloadOfTheWrongLength(int count)
    {
        var padded = Enumerable.Repeat("#FF000000", count).ToArray();

        Assert.False(ShelfPaletteWire.TryFromPaths(padded, out _));
    }

    [Fact]
    public void TryFromPaths_RefusesAValueThatIsNotAColour()
    {
        var paths = ShelfPaletteWire.ToPaths(Sample()).ToArray();
        paths[2] = "not a colour";

        Assert.False(ShelfPaletteWire.TryFromPaths(paths, out _));
    }

    /// <summary>
    /// The list is typed as IReadOnlyList of string, non-nullable under Nullable=enable, but that
    /// type describes what this side of the wire promises, not what the wire itself can
    /// guarantee: DropChannel.TryDecode builds its Paths array from whatever another process
    /// wrote, and a decoder is free to change in ways the compiler here cannot see. The null!
    /// below is deliberate, standing in for exactly that gap between the declared type and the
    /// untrusted source behind it.
    /// </summary>
    [Fact]
    public void TryFromPaths_RefusesANullElementInsteadOfThrowing()
    {
        var paths = ShelfPaletteWire.ToPaths(Sample()).ToArray();
        paths[2] = null!;

        Assert.False(ShelfPaletteWire.TryFromPaths(paths, out _));
    }
}
