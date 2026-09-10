using Plith.Services;

namespace Plith.Tests;

public class NotchPagerTests
{
    private static NotchPager Pager(int pages = 5) => new(pages);

    [Fact]
    public void StartsOnTheFirstPage()
    {
        Assert.Equal(0, Pager().Index);
    }

    [Fact]
    public void OneFullWheelDetentCommitsOnePage()
    {
        var p = Pager();

        Assert.True(p.Accumulate(NotchPager.CommitThreshold));

        Assert.Equal(1, p.Index);
    }

    [Fact]
    public void DeltaBelowTheThresholdCommitsNothing()
    {
        var p = Pager();

        Assert.False(p.Accumulate(NotchPager.CommitThreshold - 1));

        Assert.Equal(0, p.Index);
    }

    [Fact]
    public void AStreamOfSmallDeltasCommitsExactlyOnePage()
    {
        // The reason this type exists. One physical two-finger swipe emits a stream of small
        // deltas, and committing a page per delta would fly through every widget in the notch
        // before the fingers left the touchpad.
        var p = Pager();
        int committed = 0;

        for (int i = 0; i < 12; i++)
            if (p.Accumulate(40)) committed++;

        Assert.Equal(1, committed);
        Assert.Equal(1, p.Index);
    }

    [Fact]
    public void NoSecondCommitUntilTheAccumulatorFallsBackNearZero()
    {
        var p = Pager();
        Assert.True(p.Accumulate(NotchPager.CommitThreshold));

        // Still inside the same swipe: the deltas keep arriving, and none of them may page.
        for (int i = 0; i < 20; i++)
            Assert.False(p.Accumulate(60));

        Assert.Equal(1, p.Index);
    }

    [Fact]
    public void RearmsOnceTheDeltasStop()
    {
        var p = Pager();
        p.Accumulate(NotchPager.CommitThreshold);
        for (int i = 0; i < 5; i++) p.Accumulate(60);   // tail of the first swipe, ignored

        // A gap between swipes is the only thing that rearms: the caller reports it by
        // feeding deltas that fall below the rearm floor, or by calling Rest().
        p.Rest();

        Assert.True(p.Accumulate(NotchPager.CommitThreshold));
        Assert.Equal(2, p.Index);
    }

    [Fact]
    public void ADeltaBelowTheRearmFloorRearmsWithoutCommitting()
    {
        var p = Pager();
        p.Accumulate(NotchPager.CommitThreshold);

        // Inertia decaying towards zero. The small tail deltas are what tell us the swipe is
        // over, so they must rearm rather than being discarded.
        Assert.False(p.Accumulate(NotchPager.RearmFloor - 1));
        Assert.True(p.Accumulate(NotchPager.CommitThreshold));

        Assert.Equal(2, p.Index);
    }

    [Fact]
    public void NegativeDeltasPageBackwards()
    {
        var p = Pager();
        p.Accumulate(NotchPager.CommitThreshold);
        p.Rest();

        Assert.True(p.Accumulate(-NotchPager.CommitThreshold));

        Assert.Equal(0, p.Index);
    }

    [Fact]
    public void ReversingDirectionMidSwipeDoesNotCarryTheOldSign()
    {
        // Accumulating -100 then +100 must not read as a committed page in either direction:
        // the two cancel, which is what a hand that wobbled did.
        var p = Pager();

        Assert.False(p.Accumulate(-100));
        Assert.False(p.Accumulate(100));

        Assert.Equal(0, p.Index);
    }

    [Fact]
    public void IndexWrapsForwards()
    {
        var p = Pager(3);
        for (int i = 0; i < 3; i++) { p.Accumulate(NotchPager.CommitThreshold); p.Rest(); }

        Assert.Equal(0, p.Index);
    }

    [Fact]
    public void IndexWrapsBackwards()
    {
        var p = Pager(3);

        p.Accumulate(-NotchPager.CommitThreshold);

        Assert.Equal(2, p.Index);
    }

    [Fact]
    public void ASinglePageNeverMoves()
    {
        var p = Pager(1);

        Assert.False(p.Accumulate(NotchPager.CommitThreshold * 4));

        Assert.Equal(0, p.Index);
    }

    [Fact]
    public void GoToClampsAndWraps()
    {
        var p = Pager(5);

        p.GoTo(3);
        Assert.Equal(3, p.Index);

        p.GoTo(7);
        Assert.Equal(2, p.Index);

        p.GoTo(-1);
        Assert.Equal(4, p.Index);
    }

    [Fact]
    public void GoToReportsWhetherItMoved()
    {
        var p = Pager(5);

        Assert.True(p.GoTo(2));
        Assert.False(p.GoTo(2));
    }

    [Fact]
    public void GoToRearms()
    {
        // A dot click mid-swipe must leave the pager able to accept the next swipe. Without
        // this, clicking a dot while inertia was still arriving would deaden the wheel until
        // the next Rest().
        var p = Pager();
        p.Accumulate(NotchPager.CommitThreshold);

        p.GoTo(0);

        Assert.True(p.Accumulate(NotchPager.CommitThreshold));
        Assert.Equal(1, p.Index);
    }

    [Fact]
    public void ResetReturnsToTheFirstPage()
    {
        var p = Pager();
        p.GoTo(3);

        p.Reset();

        Assert.Equal(0, p.Index);
    }

    [Fact]
    public void PageCountCanChangeAndTheIndexStaysInRange()
    {
        // Widgets appear and disappear: the battery page is absent on a desktop, and the media
        // page only exists while something is playing. Shrinking the count under a high index
        // must not leave the pager pointing at a page that is gone.
        var p = Pager(5);
        p.GoTo(4);

        p.SetPageCount(3);

        Assert.Equal(3, p.PageCount);
        Assert.Equal(2, p.Index);
    }

    [Fact]
    public void PageCountBelowOneIsRejected()
    {
        var p = Pager(5);

        Assert.Throws<ArgumentOutOfRangeException>(() => p.SetPageCount(0));
    }
}
