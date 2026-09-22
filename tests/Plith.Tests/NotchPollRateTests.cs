using System.Windows;
using Plith.Views.Presentation;

namespace Plith.Tests;

public class NotchPollRateTests
{
    // The parked notch as a live build reports it: 400x68 at top-centre of a 2560 wide screen.
    private static readonly Rect Panel = new(1080, 0, 400, 68);

    private static bool Fast(Point cursor, bool buttonDown = false, Rect? panel = null) =>
        NotchPollRate.WantsFastPolling(panel ?? Panel, cursor, buttonDown);

    // ---- near the notch ----------------------------------------------------------------

    [Fact]
    public void Fast_WhenCursorIsOnTheNotch()
    {
        Assert.True(Fast(new Point(1280, 4)));
    }

    [Fact]
    public void Fast_WhenCursorIsInsideTheWatchMargin()
    {
        // Straight below the notch, well inside the margin: someone moving up towards it.
        Assert.True(Fast(new Point(1280, NotchPollRate.WatchMarginDip - 10)));
    }

    [Fact]
    public void Fast_AtTheEdgeOfTheWatchMargin()
    {
        Assert.True(Fast(new Point(1280, Panel.Bottom + NotchPollRate.WatchMarginDip - 1)));
    }

    // ---- away from the notch -----------------------------------------------------------

    [Fact]
    public void Slow_WhenCursorIsBelowTheWatchMargin()
    {
        Assert.False(Fast(new Point(1280, Panel.Bottom + NotchPollRate.WatchMarginDip + 1)));
    }

    [Fact]
    public void Slow_WhenCursorIsFarToTheSide()
    {
        Assert.False(Fast(new Point(Panel.Left - NotchPollRate.WatchMarginDip - 1, 0)));
    }

    // ---- the drag clause ---------------------------------------------------------------

    [Fact]
    public void Fast_WheneverAButtonIsDown_EvenFarAway()
    {
        // A held button anywhere is a possible drag heading for the notch, and the approach
        // detector samples its origin on the rising edge. Polling that slowly loses the origin.
        Assert.True(Fast(new Point(40, 1300), buttonDown: true));
    }

    // ---- degenerate input --------------------------------------------------------------

    [Fact]
    public void Fast_BeforeTheNotchHasBeenPositioned()
    {
        // An empty rect would put every cursor outside the band and park the poller slow
        // forever, so the notch would never answer a hover at all. Fast is the safe default.
        Assert.True(Fast(new Point(1280, 4), panel: Rect.Empty));
    }

    [Fact]
    public void Fast_WhenTheRectHasNoArea()
    {
        Assert.True(Fast(new Point(1280, 4), panel: new Rect(1080, 0, 0, 0)));
    }

    // ---- the intervals themselves ------------------------------------------------------

    [Fact]
    public void TheFastIntervalIsUnchangedFromTheAlwaysFastPoller()
    {
        // The responsive path has to stay bit-identical: this change is about when the fast
        // rate applies, never about how fast it is.
        Assert.Equal(60, NotchPollRate.Fast.TotalMilliseconds);
    }

    [Fact]
    public void TheSlowIntervalIsShortEnoughToCrossTheMargin()
    {
        // The margin exists to be crossed at the slow rate. A cursor would have to travel the
        // whole margin within one slow tick to reach the notch without ever being seen near it,
        // so the implied speed has to stay implausible for a pointing motion.
        var dipPerSecond = NotchPollRate.WatchMarginDip / NotchPollRate.Slow.TotalSeconds;
        Assert.True(dipPerSecond >= 2000,
            $"A cursor at {dipPerSecond:0} DIP/s would cross the margin unseen, which is too slow a bar.");
    }
}
