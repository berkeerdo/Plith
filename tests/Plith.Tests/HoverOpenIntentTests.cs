using System.Windows;
using Plith.Views.Presentation;

namespace Plith.Tests;

public class HoverOpenIntentTests
{
    [Fact]
    public void APageAppearingUnderAStationaryPointerDoesNotOpen()
    {
        // The rule this class exists for. Paging with the wheel leaves the pointer still, WPF
        // raises a move for the tree change, and that move must anchor rather than arm.
        var intent = new HoverOpenIntent();

        Assert.False(intent.NoteMove(new Point(100, 40)));
        Assert.False(intent.NoteMove(new Point(100, 40)));
        Assert.False(intent.Started);
    }

    [Fact]
    public void ARealMoveArmsTheDwellOnce()
    {
        var intent = new HoverOpenIntent();
        intent.NoteMove(new Point(100, 40));

        Assert.True(intent.NoteMove(new Point(140, 40)));
        Assert.True(intent.Started);

        // And not again: the dwell is already running, and re-arming on every move is how a hand
        // creeping toward a tile would never open anything at all.
        Assert.False(intent.NoteMove(new Point(160, 40)));
        Assert.False(intent.NoteMove(new Point(180, 60)));
    }

    [Fact]
    public void JitterUnderTheThresholdIsNotAMove()
    {
        var intent = new HoverOpenIntent(moveThresholdDip: 3);
        intent.NoteMove(new Point(100, 40));

        Assert.False(intent.NoteMove(new Point(102, 41)));
        Assert.False(intent.NoteMove(new Point(99, 38)));
        Assert.False(intent.Started);

        // The same hand, having actually moved.
        Assert.True(intent.NoteMove(new Point(104, 40)));
    }

    [Fact]
    public void EitherAxisOnItsOwnIsEnough()
    {
        var sideways = new HoverOpenIntent();
        sideways.NoteMove(new Point(100, 40));
        Assert.True(sideways.NoteMove(new Point(110, 40)));

        var downward = new HoverOpenIntent();
        downward.NoteMove(new Point(100, 40));
        Assert.True(downward.NoteMove(new Point(100, 50)));
    }

    [Fact]
    public void ResetReArmsFromWhereverThePointerNowIs()
    {
        // The case that would otherwise open the shelf once per session: it opened, was
        // dismissed, and the pointer never left. Reset has to make the next move an anchor
        // again, not an immediate second open.
        var intent = new HoverOpenIntent();
        intent.NoteMove(new Point(100, 40));
        Assert.True(intent.NoteMove(new Point(140, 40)));

        intent.Reset();
        Assert.False(intent.Started);

        Assert.False(intent.NoteMove(new Point(140, 40)));   // anchors again
        Assert.True(intent.NoteMove(new Point(180, 40)));    // and can arm again
    }

    [Fact]
    public void ThresholdIsMeasuredFromTheANCHORNotFromTheLastSample()
    {
        // A slow drift of one DIP per sample must still add up to a move. Measured from the last
        // sample instead, a hand moving slowly enough would never cross the threshold at all.
        var intent = new HoverOpenIntent(moveThresholdDip: 3);
        intent.NoteMove(new Point(100, 40));

        Assert.False(intent.NoteMove(new Point(101, 40)));
        Assert.False(intent.NoteMove(new Point(102, 40)));
        Assert.True(intent.NoteMove(new Point(104, 40)));
    }
}
