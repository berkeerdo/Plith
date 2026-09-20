using System.Windows;
using Plith.Views.Presentation;

namespace Plith.Tests;

public class NotchGeometryTests
{
    // The size the card content measures to with a single audio card, minus OsdContent's
    // 14 DIP drop-shadow inset — i.e. the size of the surface at full expansion, which is
    // what SetNotchMetrics hands the geometry.
    private static readonly Size Panel = new(412, 150);

    private const double RestingHeight = 5;

    // ---- SurfaceSize -------------------------------------------------------------------

    [Fact]
    public void SurfaceSize_AtZero_IsTheCollapsedPill()
    {
        var s = NotchGeometry.SurfaceSize(NotchGeometry.CollapsedWidthDip, RestingHeight, Panel, 0);
        Assert.Equal(NotchGeometry.CollapsedWidthDip, s.Width);
        Assert.Equal(RestingHeight, s.Height);
    }

    [Fact]
    public void SurfaceSize_AtOne_IsTheMeasuredPanel()
    {
        var s = NotchGeometry.SurfaceSize(NotchGeometry.CollapsedWidthDip, RestingHeight, Panel, 1);
        Assert.Equal(Panel.Width, s.Width);
        Assert.Equal(Panel.Height, s.Height);
    }

    [Fact]
    public void SurfaceSize_AtHalf_IsHalfwayOnBothAxes()
    {
        var s = NotchGeometry.SurfaceSize(NotchGeometry.CollapsedWidthDip, RestingHeight, Panel, 0.5);
        Assert.Equal((NotchGeometry.CollapsedWidthDip + Panel.Width) / 2, s.Width);
        Assert.Equal((RestingHeight + Panel.Height) / 2, s.Height);
    }

    [Fact]
    public void SurfaceSize_NeverShrinksBelowThePill_WhenNothingHasBeenMeasuredYet()
    {
        // Reachable on the first layout pass, when DesiredSize is still zero. Interpolating
        // toward a zero panel would collapse the resting pill to nothing for a frame — and at
        // rest (t = 0) it must be exactly the pill no matter what the panel measures.
        var atRest = NotchGeometry.SurfaceSize(NotchGeometry.CollapsedWidthDip, RestingHeight, new Size(0, 0), 0);
        Assert.Equal(NotchGeometry.CollapsedWidthDip, atRest.Width);
        Assert.Equal(RestingHeight, atRest.Height);

        var open = NotchGeometry.SurfaceSize(NotchGeometry.CollapsedWidthDip, RestingHeight, new Size(0, 0), 1);
        Assert.Equal(NotchGeometry.CollapsedWidthDip, open.Width);
        Assert.Equal(RestingHeight, open.Height);
    }

    [Fact]
    public void SurfaceSize_TheRestingPillDoesNotFollowThePanelWidth()
    {
        // The whole point of a fixed collapsed width: a media card appearing widens the panel,
        // and the resting shape must not move or resize when it does, or the anchor visibly
        // jumps and the notch reads as a window rather than part of the bezel.
        var narrow = NotchGeometry.SurfaceSize(NotchGeometry.CollapsedWidthDip, RestingHeight, new Size(412, 150), 0);
        var wide = NotchGeometry.SurfaceSize(NotchGeometry.CollapsedWidthDip, RestingHeight, new Size(1172, 260), 0);
        Assert.Equal(narrow, wide);
    }

    [Fact]
    public void SurfaceSize_ClampsProgressOutsideZeroToOne()
    {
        // The window is sized to the open panel exactly, so a surface larger than the panel
        // would be clipped rather than seen. An easing function that overshoots must not be
        // able to produce one.
        var over = NotchGeometry.SurfaceSize(NotchGeometry.CollapsedWidthDip, RestingHeight, Panel, 1.4);
        Assert.Equal(Panel.Width, over.Width);

        var under = NotchGeometry.SurfaceSize(NotchGeometry.CollapsedWidthDip, RestingHeight, Panel, -0.3);
        Assert.Equal(NotchGeometry.CollapsedWidthDip, under.Width);
    }

    // ---- SurfaceRadius -----------------------------------------------------------------

    [Fact]
    public void SurfaceRadius_GrowsWithExpansion()
    {
        Assert.Equal(NotchGeometry.CollapsedRadiusDip, NotchGeometry.SurfaceRadius(0, surfaceHeight: 40));
        Assert.Equal(NotchGeometry.ExpandedRadiusDip, NotchGeometry.SurfaceRadius(1, surfaceHeight: 150));
    }

    [Fact]
    public void SurfaceRadius_NeverExceedsTheSurfaceHeight()
    {
        // The resting height is a user setting that goes down to 2 DIP. An un-clamped 8 DIP
        // radius on a 2 DIP tall shape is a radius the shape cannot express: WPF clamps it at
        // draw time, so the drawn corner would stop tracking the value the animation holds.
        Assert.Equal(2, NotchGeometry.SurfaceRadius(0, surfaceHeight: 2));
    }

    [Fact]
    public void SurfaceRadius_NeverGoesNegative_ForADegenerateSurface()
    {
        Assert.Equal(0, NotchGeometry.SurfaceRadius(0, surfaceHeight: -5));
    }

    // ---- ContentOpacity ----------------------------------------------------------------

    [Fact]
    public void ContentOpacity_IsZeroWhileTheShapeIsStillGrowing()
    {
        // The ordering IS the effect: the shape settles first, the content arrives into it.
        // Fading content in while the surface is still moving reads as a window resizing.
        Assert.Equal(0, NotchGeometry.ContentOpacity(0));
        Assert.Equal(0, NotchGeometry.ContentOpacity(0.3));
        Assert.Equal(0, NotchGeometry.ContentOpacity(NotchGeometry.ContentFadeStart));
    }

    [Fact]
    public void ContentOpacity_ReachesFullOnlyWhenFullyOpen()
    {
        Assert.Equal(1, NotchGeometry.ContentOpacity(1));
        Assert.True(NotchGeometry.ContentOpacity(0.9) < 1);
    }

    [Fact]
    public void ContentOpacity_RampsMonotonically_AfterTheFadeStart()
    {
        var a = NotchGeometry.ContentOpacity(0.7);
        var b = NotchGeometry.ContentOpacity(0.85);
        Assert.True(a > 0 && b > a, $"expected a rising ramp, got {a} then {b}");
    }

    [Fact]
    public void ContentOpacity_StaysInRange_ForProgressOutsideZeroToOne()
    {
        Assert.Equal(0, NotchGeometry.ContentOpacity(-1));
        Assert.Equal(1, NotchGeometry.ContentOpacity(2));
    }

    // ---- HoverRect ---------------------------------------------------------------------

    [Fact]
    public void HoverRect_IsCentredInTheWindowAtItsTop()
    {
        var r = NotchGeometry.HoverRect(windowLeft: 740, windowTop: 0, windowWidth: 440, collapsedHeight: 20);

        Assert.Equal(740 + (440 - NotchGeometry.CollapsedWidthDip) / 2, r.Left);
        Assert.Equal(0, r.Top);
        Assert.Equal(NotchGeometry.CollapsedWidthDip, r.Width);
        Assert.Equal(20, r.Height);
    }

    [Fact]
    public void HoverRect_FollowsTheWindowOrigin()
    {
        // The window origin comes from Reposition(), which anchors on Screen.WorkingArea.
        // A taskbar docked to the top therefore moves the target down with it, and this is
        // the only thing NotchGeometry needs to know about that.
        var r = NotchGeometry.HoverRect(740, 48, 440, 20);
        Assert.Equal(48, r.Top);
    }

    [Fact]
    public void HoverRect_StaysPutWhenThePanelWidens()
    {
        // Centre of the pill, before and after a media card widens the window. Reposition()
        // re-centres the window on the same screen, so a wider window starts further left by
        // exactly half the extra width — and the pill's centre must not move at all.
        var narrow = NotchGeometry.HoverRect(windowLeft: 1060, windowTop: 0, windowWidth: 440, collapsedHeight: 20);
        var wide = NotchGeometry.HoverRect(windowLeft: 680, windowTop: 0, windowWidth: 1200, collapsedHeight: 20);

        Assert.Equal(narrow.Left + narrow.Width / 2, wide.Left + wide.Width / 2);
        Assert.Equal(narrow.Width, wide.Width);
    }

    [Fact]
    public void HoverRect_IsTallerThanADegenerateRestingHeight()
    {
        // The resting height goes down to 2 DIP, and a 2 DIP tall target cannot be hit
        // deliberately — the pointer skips it between two mouse samples. The target is
        // invisible, so making it taller than what is drawn costs nothing on screen.
        var r = NotchGeometry.HoverRect(740, 0, 440, collapsedHeight: 2);
        Assert.Equal(NotchGeometry.MinHoverHeightDip, r.Height);
    }

    [Fact]
    public void HoverRect_DoesNotOverflowAWindowNarrowerThanThePill()
    {
        var r = NotchGeometry.HoverRect(0, 0, windowWidth: 100, collapsedHeight: 20);
        Assert.Equal(0, r.Left);
        Assert.Equal(100, r.Width);
    }

    // ---- PhysicalToDip -----------------------------------------------------------------

    [Fact]
    public void PhysicalToDip_At100Percent_IsIdentity()
    {
        Assert.Equal(new Point(800, 12), NotchGeometry.PhysicalToDip(800, 12, 1.0));
    }

    [Fact]
    public void PhysicalToDip_At125Percent_DividesOut()
    {
        // GetCursorPos reports physical pixels; Left/Top and WorkingArea are DIP. This is
        // the conversion the spec requires to be explicit and covered at a non-100 % scale.
        Assert.Equal(new Point(800, 12), NotchGeometry.PhysicalToDip(1000, 15, 1.25));
    }

    [Fact]
    public void PhysicalToDip_At150Percent_DividesOut()
    {
        Assert.Equal(new Point(640, 8), NotchGeometry.PhysicalToDip(960, 12, 1.5));
    }

    [Fact]
    public void PhysicalToDip_TreatsANonPositiveScaleAsOneToOne()
    {
        // A failed DPI query must not divide by zero and teleport the cursor to infinity.
        Assert.Equal(new Point(800, 12), NotchGeometry.PhysicalToDip(800, 12, 0));
    }

    // ---- IsInsideNotch -----------------------------------------------------------------

    [Fact]
    public void IsInsideNotch_JustInside()
    {
        var r = NotchGeometry.HoverRect(740, 0, 440, 20);
        Assert.True(NotchGeometry.IsInsideNotch(r, new Point(960, 2)));
    }

    [Fact]
    public void IsInsideNotch_JustBelow()
    {
        var r = NotchGeometry.HoverRect(740, 0, 440, 20);
        Assert.False(NotchGeometry.IsInsideNotch(r, new Point(960, 21)));
    }

    [Fact]
    public void IsInsideNotch_JustLeftOfIt()
    {
        var r = NotchGeometry.HoverRect(740, 0, 440, 20);
        Assert.False(NotchGeometry.IsInsideNotch(r, new Point(r.Left - 1, 2)));
    }

    /// <summary>
    /// The guarantee the flat shelf rests on: the cap is exactly what the surface can draw, so no
    /// file can be on the shelf and off the screen. Written as a test rather than left to the
    /// definition because a later edit could reintroduce a free-standing number.
    /// </summary>
    [Fact]
    public void ShelfCapacityIsExactlyWhatTheGridDraws()
    {
        Assert.Equal(NotchGeometry.ShelfTilesPerRow * NotchGeometry.ShelfRowCount,
                     NotchGeometry.ShelfCapacity);
        Assert.Equal(15, NotchGeometry.ShelfCapacity);
    }

    /// <summary>
    /// The frame must be tall enough for every row it promises. A height typed as a literal is
    /// free to stop matching the row count above it, which is the failure this derivation exists
    /// to make impossible.
    /// </summary>
    [Fact]
    public void ShelfFrameIsTallEnoughForEveryRow()
    {
        var rows = NotchGeometry.ShelfRowCount * NotchGeometry.ShelfTileSize
                 + (NotchGeometry.ShelfRowCount - 1) * NotchGeometry.ShelfGap;

        Assert.True(NotchGeometry.ShelfFrameDip.Height >= rows,
            $"frame {NotchGeometry.ShelfFrameDip.Height} is shorter than its " +
            $"{NotchGeometry.ShelfRowCount} rows ({rows})");
    }

    /// <summary>
    /// The shelf hugs what it holds. Two files must not open a pane sized for fifteen, and the
    /// row count is what the frame height is built from.
    /// </summary>
    [Fact]
    public void ShelfRowsFollowTheItemCount()
    {
        // An empty shelf gets TWO rows, which is more than anything it holds needs.
        //
        // Not an oversight and not symmetry for its own sake: the empty state is the only thing
        // the shelf ever draws that is not a tile. It carries an icon over a line of text, and at
        // one row the box around it is 352 by 64, which is the aspect ratio of a text input. It
        // read as a field to type in rather than a place to drop onto, reported exactly that way
        // from the running build.
        //
        // The shelf cannot gain items while it is open (see ShelfFrameFor), so nobody ever
        // watches it shrink from two rows to one; the next open is simply the right size for what
        // is on it.
        Assert.Equal(2, NotchGeometry.ShelfRowsFor(0));
        Assert.Equal(1, NotchGeometry.ShelfRowsFor(1));
        Assert.Equal(1, NotchGeometry.ShelfRowsFor(NotchGeometry.ShelfTilesPerRow));
        Assert.Equal(2, NotchGeometry.ShelfRowsFor(NotchGeometry.ShelfTilesPerRow + 1));
        Assert.Equal(NotchGeometry.ShelfRowCount, NotchGeometry.ShelfRowsFor(NotchGeometry.ShelfCapacity));
    }

    /// <summary>
    /// A count past the cap cannot ask for a fourth row. The store enforces the cap, but this
    /// number arrives from a caller and the ceiling is what keeps the frame inside the design.
    /// </summary>
    [Fact]
    public void ShelfRowsAreClampedToTheCeiling()
    {
        Assert.Equal(NotchGeometry.ShelfRowCount, NotchGeometry.ShelfRowsFor(NotchGeometry.ShelfCapacity + 40));
        Assert.Equal(2, NotchGeometry.ShelfRowsFor(-3));
    }

    /// <summary>
    /// A full shelf is exactly the frame the ceiling describes, and a smaller one is genuinely
    /// smaller. The first half is what stops the two definitions drifting; the second is the
    /// whole point of sizing to the contents.
    /// </summary>
    [Fact]
    public void ShelfFrameForAFullShelfIsTheCeilingFrame()
    {
        Assert.Equal(NotchGeometry.ShelfFrameDip.Height,
                     NotchGeometry.ShelfFrameFor(NotchGeometry.ShelfCapacity).Height);
        Assert.True(NotchGeometry.ShelfFrameFor(2).Height < NotchGeometry.ShelfFrameDip.Height,
            "a two-file shelf should not open a pane sized for a full one");
    }

    /// <summary>
    /// A row of tiles plus the gaps between them must fit the frame's width, which is unchanged
    /// at 384. Five 64 DIP tiles with four 8 DIP gaps is 352, leaving 32 for the horizontal
    /// chrome.
    /// </summary>
    [Fact]
    public void ShelfFrameIsWideEnoughForARow()
    {
        var row = NotchGeometry.ShelfTilesPerRow * NotchGeometry.ShelfTileSize
                + (NotchGeometry.ShelfTilesPerRow - 1) * NotchGeometry.ShelfGap;

        Assert.True(NotchGeometry.ShelfFrameDip.Width >= row,
            $"frame {NotchGeometry.ShelfFrameDip.Width} is narrower than one row ({row})");
    }

    [Fact]
    public void OutputPickerCapacityIsTheProductOfItsGrid()
    {
        // Defined as the product for the same reason ShelfCapacity is: a literal beside a grid is
        // free to stop matching it, and this is the number that decides whether a device is drawn
        // at all.
        Assert.Equal(NotchGeometry.OutputPickerColumns * NotchGeometry.OutputPickerRows,
                     NotchGeometry.OutputPickerCapacity);
    }
}
