using Plith.Services;

namespace Plith.Tests;

public class WheelDecoderTests
{
    /// <summary>Build a wParam the way Windows does: delta in the high word, key flags in the low.</summary>
    private static nint W(int delta, int keys = 0) =>
        (nint)(((ulong)(ushort)(short)delta << 16) | (uint)keys);

    private const int Shift = 0x0004;

    [Fact]
    public void HorizontalWheelPassesThroughUnchanged()
    {
        Assert.Equal(120, WheelDecoder.TryDecode(WheelDecoder.WM_MOUSEHWHEEL, W(120)));
    }

    [Fact]
    public void HorizontalWheelKeepsItsSign()
    {
        // The high word is a SIGNED short. Read as unsigned it would come back as 65416, which
        // is both the wrong direction and far past any commit threshold.
        Assert.Equal(-120, WheelDecoder.TryDecode(WheelDecoder.WM_MOUSEHWHEEL, W(-120)));
    }

    [Fact]
    public void ShiftWheelIsNegated()
    {
        // Positive WM_MOUSEWHEEL is forward, away from the hand, which scrolls left by the
        // Windows convention - so it is the previous page, not the next one.
        Assert.Equal(-120, WheelDecoder.TryDecode(WheelDecoder.WM_MOUSEWHEEL, W(120, Shift)));
        Assert.Equal(120, WheelDecoder.TryDecode(WheelDecoder.WM_MOUSEWHEEL, W(-120, Shift)));
    }

    [Fact]
    public void PlainVerticalWheelIsNotAPagingGesture()
    {
        // Null, not 0: the caller has to be able to leave the message unhandled so it reaches
        // whatever else wants it.
        Assert.Null(WheelDecoder.TryDecode(WheelDecoder.WM_MOUSEWHEEL, W(120)));
    }

    [Fact]
    public void OtherMessagesAreIgnored()
    {
        Assert.Null(WheelDecoder.TryDecode(0x0200, W(120)));
    }

    [Fact]
    public void OtherModifiersDoNotEnableTheShiftPath()
    {
        const int ctrl = 0x0008;
        Assert.Null(WheelDecoder.TryDecode(WheelDecoder.WM_MOUSEWHEEL, W(120, ctrl)));
    }

    [Fact]
    public void ShiftAlongsideOtherModifiersStillCounts()
    {
        const int ctrl = 0x0008;
        Assert.Equal(-120, WheelDecoder.TryDecode(WheelDecoder.WM_MOUSEWHEEL, W(120, Shift | ctrl)));
    }

    [Fact]
    public void HorizontalWheelIgnoresModifiers()
    {
        Assert.Equal(120, WheelDecoder.TryDecode(WheelDecoder.WM_MOUSEHWHEEL, W(120, Shift)));
    }

    [Fact]
    public void ASmallPartialDeltaSurvivesTheRoundTrip()
    {
        // Touchpads emit fractions of a detent. These are the values the pager accumulates, so
        // they must not be rounded away here.
        Assert.Equal(17, WheelDecoder.TryDecode(WheelDecoder.WM_MOUSEHWHEEL, W(17)));
        Assert.Equal(-3, WheelDecoder.TryDecode(WheelDecoder.WM_MOUSEHWHEEL, W(-3)));
    }
}
