using Plith.Services;

namespace Plith.Tests;

public class NotchPagerTests
{
    /// <summary>
    /// A pager plus a clock, so a test reads as one continuous gesture unless it says
    /// otherwise. Each delta advances the clock by less than <see cref="NotchPager.IdleRearmMs"/>,
    /// which is what "the same swipe" means.
    /// </summary>
    private sealed class Gesture(int pages = 5)
    {
        public NotchPager Pager { get; } = new(pages);
        private long _now;

        public bool Feed(int delta)
        {
            _now += 10;
            return Pager.Accumulate(delta, _now);
        }

        /// <summary>Let the wheel go quiet for longer than the idle gap.</summary>
        public void Pause() => _now += NotchPager.IdleRearmMs;

        public int Index => Pager.Index;
    }

    [Fact]
    public void StartsOnTheFirstPage()
    {
        Assert.Equal(0, new Gesture().Index);
    }

    [Fact]
    public void OneFullWheelDetentCommitsOnePage()
    {
        var g = new Gesture();

        Assert.True(g.Feed(NotchPager.CommitThreshold));

        Assert.Equal(1, g.Index);
    }

    [Fact]
    public void DeltaBelowTheThresholdCommitsNothing()
    {
        var g = new Gesture();

        Assert.False(g.Feed(NotchPager.CommitThreshold - 1));

        Assert.Equal(0, g.Index);
    }

    [Fact]
    public void AStreamOfSmallDeltasCommitsExactlyOnePage()
    {
        // The reason this type exists. One physical two-finger swipe emits a stream of small
        // deltas, and committing a page per delta would fly through every widget in the notch
        // before the fingers left the touchpad.
        var g = new Gesture();
        int committed = 0;

        for (int i = 0; i < 12; i++)
            if (g.Feed(40)) committed++;

        Assert.Equal(1, committed);
        Assert.Equal(1, g.Index);
    }

    [Fact]
    public void NoSecondCommitWhileTheSameSwipeKeepsSendingLargeDeltas()
    {
        var g = new Gesture();
        Assert.True(g.Feed(NotchPager.CommitThreshold));

        for (int i = 0; i < 20; i++)
            Assert.False(g.Feed(60));

        Assert.Equal(1, g.Index);
    }

    [Fact]
    public void ADeltaBelowTheRearmFloorRearmsWithoutCommitting()
    {
        var g = new Gesture();
        g.Feed(NotchPager.CommitThreshold);

        // Inertia decaying towards zero. Those small tail deltas are what tell us the swipe is
        // over, so they must rearm rather than being discarded.
        Assert.False(g.Feed(NotchPager.RearmFloor - 1));
        Assert.True(g.Feed(NotchPager.CommitThreshold));

        Assert.Equal(2, g.Index);
    }

    [Fact]
    public void AQuietWheelRearmsEvenWhenEveryDeltaIsAFullDetent()
    {
        // A mouse's tilt wheel sends exactly one WHEEL_DELTA per detent and never anything
        // smaller, so the rearm floor alone can never fire for it. Without the idle gap this
        // pager would page once and then stay deaf for the rest of the session.
        var g = new Gesture();

        Assert.True(g.Feed(NotchPager.CommitThreshold));
        g.Pause();
        Assert.True(g.Feed(NotchPager.CommitThreshold));
        g.Pause();
        Assert.True(g.Feed(NotchPager.CommitThreshold));

        Assert.Equal(3, g.Index);
    }

    [Fact]
    public void TheIdleGapDoesNotFireInsideOneSwipe()
    {
        // The other side of the same rule: deltas arriving 10 ms apart are one gesture, and a
        // gap rule loose enough to break them apart would page several times per swipe.
        var g = new Gesture();
        int committed = 0;

        for (int i = 0; i < 30; i++)
            if (g.Feed(60)) committed++;

        Assert.Equal(1, committed);
    }

    [Fact]
    public void TheFirstEverDeltaIsNotTreatedAsContinuingAnything()
    {
        var g = new Gesture();

        Assert.True(g.Feed(NotchPager.CommitThreshold));
    }

    [Fact]
    public void NegativeDeltasPageBackwards()
    {
        var g = new Gesture();
        g.Feed(NotchPager.CommitThreshold);
        g.Pause();

        Assert.True(g.Feed(-NotchPager.CommitThreshold));

        Assert.Equal(0, g.Index);
    }

    [Fact]
    public void ReversingDirectionMidSwipeDoesNotCarryTheOldSign()
    {
        // -100 then +100 must not read as a committed page in either direction: the two cancel,
        // which is what a hand that wobbled did.
        var g = new Gesture();

        Assert.False(g.Feed(-100));
        Assert.False(g.Feed(100));

        Assert.Equal(0, g.Index);
    }

    [Fact]
    public void IndexWrapsForwards()
    {
        var g = new Gesture(3);

        for (int i = 0; i < 3; i++) { g.Feed(NotchPager.CommitThreshold); g.Pause(); }

        Assert.Equal(0, g.Index);
    }

    [Fact]
    public void IndexWrapsBackwards()
    {
        var g = new Gesture(3);

        g.Feed(-NotchPager.CommitThreshold);

        Assert.Equal(2, g.Index);
    }

    [Fact]
    public void ASinglePageNeverMoves()
    {
        var g = new Gesture(1);

        Assert.False(g.Feed(NotchPager.CommitThreshold * 4));

        Assert.Equal(0, g.Index);
    }

    [Fact]
    public void GoToClampsAndWraps()
    {
        var p = new NotchPager(5);

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
        var p = new NotchPager(5);

        Assert.True(p.GoTo(2));
        Assert.False(p.GoTo(2));
    }

    [Fact]
    public void GoToRearms()
    {
        // A dot clicked while inertia was still arriving must leave the wheel usable. Without
        // this the pager stays deaf until the next quiet moment.
        var g = new Gesture();
        g.Feed(NotchPager.CommitThreshold);

        g.Pager.GoTo(0);

        Assert.True(g.Feed(NotchPager.CommitThreshold));
        Assert.Equal(1, g.Index);
    }

    [Fact]
    public void ResetReturnsToTheFirstPage()
    {
        var p = new NotchPager(5);
        p.GoTo(3);

        p.Reset();

        Assert.Equal(0, p.Index);
    }

    [Fact]
    public void ResetForgetsTheGestureTiming()
    {
        // The notch closed. A swipe an hour later must not be measured against the timestamp of
        // the last one — and, more to the point, must not be treated as continuing it.
        var p = new NotchPager(5);
        p.Accumulate(NotchPager.CommitThreshold, 1_000);

        p.Reset();

        Assert.True(p.Accumulate(NotchPager.CommitThreshold, 1_005));
        Assert.Equal(1, p.Index);
    }

    [Fact]
    public void PageCountCanChangeAndTheIndexStaysInRange()
    {
        // Widgets appear and disappear: the media page only exists while something is playing.
        // Shrinking the count under a high index must not leave the pager pointing at a page
        // that is gone.
        var p = new NotchPager(5);
        p.GoTo(4);

        p.SetPageCount(3);

        Assert.Equal(3, p.PageCount);
        Assert.Equal(2, p.Index);
    }

    [Fact]
    public void PageCountBelowOneIsRejected()
    {
        var p = new NotchPager(5);

        Assert.Throws<ArgumentOutOfRangeException>(() => p.SetPageCount(0));
    }

    [Fact]
    public void ConstructingWithNoPagesIsRejected()
    {
        Assert.Throws<ArgumentOutOfRangeException>(() => new NotchPager(0));
    }
}
