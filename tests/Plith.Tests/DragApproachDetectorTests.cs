using Plith.Views.Presentation;

namespace Plith.Tests;

public class DragApproachDetectorTests
{
    [Fact]
    public void ADragThatBeganElsewhereAndReachesTheBandIsApproaching()
    {
        var detector = new DragApproachDetector();

        Assert.False(detector.Update(buttonDown: true, cursorOverOsd: false,
                                     cursorInApproachBand: false, cursorInHoldBand: false));
        Assert.True(detector.Update(buttonDown: true, cursorOverOsd: false,
                                    cursorInApproachBand: true, cursorInHoldBand: true));
        Assert.True(detector.IsApproaching);
    }

    /// <summary>A press that began ON the notch is a click, whatever it does afterwards. Without
    /// this every click on the notch would hand the screen to the catcher.</summary>
    [Fact]
    public void APressThatBeganOnTheOsdIsNeverADrag()
    {
        var detector = new DragApproachDetector();

        detector.Update(buttonDown: true, cursorOverOsd: true,
                        cursorInApproachBand: true, cursorInHoldBand: true);

        Assert.False(detector.IsApproaching);
        Assert.False(detector.Update(buttonDown: true, cursorOverOsd: true,
                                     cursorInApproachBand: true, cursorInHoldBand: true));
    }

    /// <summary>
    /// The origin is sampled once, on the rising edge. Re-read every tick it would flip to
    /// "inside" the instant the cursor arrived — which is the one moment it still has to
    /// describe where the press began — and the drag would never register.
    /// </summary>
    [Fact]
    public void TheOriginIsFixedAtTheMomentTheButtonWentDown()
    {
        var detector = new DragApproachDetector();

        detector.Update(buttonDown: true, cursorOverOsd: false,
                        cursorInApproachBand: false, cursorInHoldBand: false);
        // Now over the OSD, as any drag that reaches the notch must be.
        detector.Update(buttonDown: true, cursorOverOsd: true,
                        cursorInApproachBand: true, cursorInHoldBand: true);

        Assert.True(detector.IsApproaching);
    }

    /// <summary>
    /// The defect this split exists for, measured on a live drag before it was written.
    ///
    /// The handoff starts on a 190x28 DIP band at the top of the screen and opens a 356x116
    /// panel. Aiming a drop inside that panel means moving out of the band — so with a single
    /// threshold the notch came back 700 ms into the drag, and the catcher vanished from under a
    /// file still in the air.
    /// </summary>
    [Fact]
    public void OnceApproachingItIsTheWIDERRectangleThatKeepsItGoing()
    {
        var detector = new DragApproachDetector();
        detector.Update(buttonDown: true, cursorOverOsd: false,
                        cursorInApproachBand: true, cursorInHoldBand: true);

        // Down into the open panel: out of the band that started this, still on the target.
        Assert.False(detector.Update(buttonDown: true, cursorOverOsd: true,
                                     cursorInApproachBand: false, cursorInHoldBand: true));
        Assert.True(detector.IsApproaching);
    }

    /// <summary>The narrow band is what STARTS one, though — being merely somewhere in the wider
    /// rectangle must not begin a handoff, or every gesture near the top of the screen would.</summary>
    [Fact]
    public void TheWiderRectangleAloneDoesNotStartOne()
    {
        var detector = new DragApproachDetector();

        Assert.False(detector.Update(buttonDown: true, cursorOverOsd: false,
                                     cursorInApproachBand: false, cursorInHoldBand: true));
        Assert.False(detector.IsApproaching);
    }

    [Fact]
    public void ReleasingEndsTheApproach()
    {
        var detector = new DragApproachDetector();
        detector.Update(buttonDown: true, cursorOverOsd: false,
                        cursorInApproachBand: true, cursorInHoldBand: true);

        Assert.True(detector.Update(buttonDown: false, cursorOverOsd: true,
                                    cursorInApproachBand: true, cursorInHoldBand: true));
        Assert.False(detector.IsApproaching);
    }

    [Fact]
    public void LeavingTheTargetAltogetherEndsTheApproach()
    {
        var detector = new DragApproachDetector();
        detector.Update(buttonDown: true, cursorOverOsd: false,
                        cursorInApproachBand: true, cursorInHoldBand: true);

        Assert.True(detector.Update(buttonDown: true, cursorOverOsd: false,
                                    cursorInApproachBand: false, cursorInHoldBand: false));
        Assert.False(detector.IsApproaching);
    }

    [Fact]
    public void ATransitionIsReportedOnceRatherThanEveryTick()
    {
        var detector = new DragApproachDetector();
        detector.Update(buttonDown: true, cursorOverOsd: false,
                        cursorInApproachBand: false, cursorInHoldBand: false);

        Assert.True(detector.Update(buttonDown: true, cursorOverOsd: false,
                                    cursorInApproachBand: true, cursorInHoldBand: true));
        Assert.False(detector.Update(buttonDown: true, cursorOverOsd: false,
                                     cursorInApproachBand: true, cursorInHoldBand: true));
        Assert.False(detector.Update(buttonDown: true, cursorOverOsd: false,
                                     cursorInApproachBand: true, cursorInHoldBand: true));
    }

    /// <summary>
    /// A poller stopped mid-drag and restarted later must not believe a button it never saw go
    /// down is still held — the first cursor to cross the band would hand the screen to the
    /// catcher with no drag in flight at all.
    /// </summary>
    [Fact]
    public void ResetForgetsAHeldButton()
    {
        var detector = new DragApproachDetector();
        detector.Update(buttonDown: true, cursorOverOsd: false,
                        cursorInApproachBand: true, cursorInHoldBand: true);

        detector.Reset();

        Assert.False(detector.IsApproaching);
        // Still held, but this detector never saw it go down, so the origin is unknown and the
        // safe reading is "not a drag" until the next press.
        Assert.False(detector.Update(buttonDown: true, cursorOverOsd: true,
                                     cursorInApproachBand: true, cursorInHoldBand: true));
        Assert.False(detector.IsApproaching);
    }
}
