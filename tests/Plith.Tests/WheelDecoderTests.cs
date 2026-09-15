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
    public void PlainVerticalWheelPagesToo()
    {
        // It did not at first, on the principle that a vertical wheel is not a sideways gesture.
        // The notch has nothing of its own to scroll, so a wheel delivered to it can only have
        // been meant for it - and a rule that makes someone hold a modifier to use the obvious
        // control has no beneficiary.
        Assert.Equal(-120, WheelDecoder.TryDecode(WheelDecoder.WM_MOUSEWHEEL, W(120)));
    }

    [Fact]
    public void ForwardIsBackwards()
    {
        // Forward, away from the hand, moves toward the START of a list on every surface a
        // person has ever scrolled. Both vertical paths agree on that.
        Assert.True(WheelDecoder.TryDecode(WheelDecoder.WM_MOUSEWHEEL, W(120)) < 0);
        Assert.True(WheelDecoder.TryDecode(WheelDecoder.WM_MOUSEWHEEL, W(120, Shift)) < 0);
    }

    [Fact]
    public void OtherMessagesAreIgnored()
    {
        Assert.Null(WheelDecoder.TryDecode(0x0200, W(120)));
    }

    [Fact]
    public void ModifiersDoNotChangeTheVerticalResult()
    {
        // Shift used to be what made a vertical wheel count. Now it changes nothing, which is
        // the honest consequence of the plain wheel already working.
        const int ctrl = 0x0008;
        Assert.Equal(-120, WheelDecoder.TryDecode(WheelDecoder.WM_MOUSEWHEEL, W(120, ctrl)));
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
