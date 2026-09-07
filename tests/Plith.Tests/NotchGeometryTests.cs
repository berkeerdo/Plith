using System.Windows;
using Plith.Views.Presentation;

namespace Plith.Tests;

public class NotchGeometryTests
{
    // OsdContent's outer Grid has Margin="14" to reserve drop-shadow space, so the card's
    // visible border starts 14 DIP below the content origin. Every case here uses that
    // real value rather than 0, because a geometry that only works at inset 0 would look
    // correct in tests and sit 14 px too low on screen.
    private const double Inset = 14;

    [Fact]
    public void HiddenOffset_TakesTheWholeCardOffScreen()
    {
        // 200 tall content, 14 inset: push up until the card's bottom edge lands at y = 0, so
        // the only thing left on screen is whatever the strip element itself draws.
        Assert.Equal(-186, NotchGeometry.HiddenOffset(contentHeight: 200, contentInset: Inset));
    }

    [Fact]
    public void HiddenOffset_ScalesWithContentHeight()
    {
        Assert.Equal(-136, NotchGeometry.HiddenOffset(150, Inset));
        Assert.Equal(-286, NotchGeometry.HiddenOffset(300, Inset));
    }

    [Fact]
    public void HiddenOffset_NeverPushesDown_ForDegenerateContent()
    {
        // Reachable during the first layout pass, when DesiredSize is still zero. A positive
        // offset there would drop the card into the middle of the screen for one frame.
        Assert.Equal(0, NotchGeometry.HiddenOffset(contentHeight: 0, contentInset: Inset));
    }

    [Fact]
    public void DescendedOffset_IsZero()
    {
        Assert.Equal(0.0, NotchGeometry.DescendedOffset);
    }

    [Fact]
    public void HiddenOffset_TakesTheCardFullyOffScreen()
    {
        // 200 tall content: push up until only the inset (the drop-shadow margin) remains
        // below the origin, i.e. nothing of the card's visible border is left on screen.
        Assert.Equal(-186, NotchGeometry.HiddenOffset(contentHeight: 200, contentInset: Inset));
    }

    [Fact]
    public void HiddenOffset_NeverPushesDown_WhenContentIsShorterThanTheInset()
    {
        Assert.Equal(0, NotchGeometry.HiddenOffset(contentHeight: 0, contentInset: Inset));
    }

    // Deleted: HiddenOffset_DoesNotDependOnStripHeight. It called HiddenOffset twice with
    // identical arguments and asserted the two results equal, which is true of any pure
    // function, then repeated the -186 assertion already made above. The property it claimed
    // to cover — that strip height never enters the card's placement — is enforced by the
    // signature: HiddenOffset takes content height and inset and nothing else, so no test can
    // vary a strip height it cannot be given.

    [Fact]
    public void HiddenOffset_LeavesTheCardsBottomEdgeAtZero()
    {
        // The card's bottom edge, in the content's own coordinate space, sits at
        // contentHeight - contentInset below the content origin (the inset is the drop-shadow
        // margin the visible border starts inside of). Parking at HiddenOffset must bring that
        // edge to exactly y = 0 — the top of the window — so nothing of the card draws above
        // or below it.
        const double contentHeight = 200;
        var hidden = NotchGeometry.HiddenOffset(contentHeight, Inset);
        Assert.Equal(0, hidden + (contentHeight - Inset));
    }

    [Fact]
    public void StripRect_SitsAtTheWindowTopAndInsideTheShadowInset()
    {
        var r = NotchGeometry.StripRect(
            windowLeft: 740, windowTop: 0, contentWidth: 440, stripHeight: 5, contentInset: Inset);

        Assert.Equal(754, r.Left);    // 740 + 14
        Assert.Equal(0, r.Top);
        Assert.Equal(412, r.Width);   // 440 - 14 * 2
        Assert.Equal(5, r.Height);
    }

    [Fact]
    public void StripRect_FollowsTheWindowOrigin()
    {
        // The window origin comes from Reposition(), which anchors on Screen.WorkingArea.
        // A taskbar docked to the top therefore moves the strip down with it, and this is
        // the only thing NotchGeometry needs to know about that.
        var r = NotchGeometry.StripRect(740, 48, 440, 5, Inset);
        Assert.Equal(48, r.Top);
    }

    [Fact]
    public void StripRect_DoesNotGoNegative_ForContentNarrowerThanTheInset()
    {
        var r = NotchGeometry.StripRect(0, 0, contentWidth: 10, stripHeight: 5, contentInset: Inset);
        Assert.Equal(0, r.Width);
    }

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

    [Fact]
    public void IsInsideStrip_JustInside()
    {
        var r = NotchGeometry.StripRect(740, 0, 440, 5, Inset);
        Assert.True(NotchGeometry.IsInsideStrip(r, new Point(755, 2)));
    }

    [Fact]
    public void IsInsideStrip_JustBelow()
    {
        var r = NotchGeometry.StripRect(740, 0, 440, 5, Inset);
        Assert.False(NotchGeometry.IsInsideStrip(r, new Point(755, 6)));
    }

    [Fact]
    public void IsInsideStrip_JustLeftOfIt()
    {
        var r = NotchGeometry.StripRect(740, 0, 440, 5, Inset);
        Assert.False(NotchGeometry.IsInsideStrip(r, new Point(753, 2)));
    }
}
