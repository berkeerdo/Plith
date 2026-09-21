using System.Windows;

namespace Plith.Views.Presentation;

/// <summary>
/// Decides when a pointer over a page means "open it", as opposed to a pointer that merely
/// happens to be there.
///
/// THE DISTINCTION IS THE WHOLE POINT, and it exists because of how the notch is used. Paging
/// through widgets is done with the wheel, and the pointer sits still over the frame while the
/// pages change under it. A page that opened on arrival would open every time someone paged past
/// it, which is not a hover, it is a coincidence. So a real pointer MOVEMENT over the page is
/// required first, and only then does the dwell begin.
///
/// Why any of this is needed at all, for the shelf: a file can only be dragged out of the
/// catcher's window, never Plith's, because Plith runs at high integrity in a Release build and
/// DoDragDrop carries nothing from there (measured in docs/SHELF-VERIFICATION.md section 4).
/// Delegating the drag was measured too, and it is a dead end: a drag is never delivered for a
/// press that happened in another process. So the shelf has to be OPEN before the press lands,
/// which means opening on hover rather than on click. Reported from a real install: the first
/// person to try it grabbed a tile in the notch and got the shelf opening instead of a drag.
///
/// Pure, and in Plith rather than in the widget, because the test suite is not STA and cannot
/// construct a UserControl. The timer stays in the view; the rule lives here.
/// </summary>
public sealed class HoverOpenIntent
{
    private Point? _anchor;
    private bool _started;

    /// <param name="moveThresholdDip">How far the pointer must travel before it counts as a
    /// movement. Three DIP: enough that the sub-pixel jitter a still hand produces does not
    /// qualify, small enough that nudging the pointer onto a tile does.</param>
    public HoverOpenIntent(double moveThresholdDip = 3) => MoveThresholdDip = moveThresholdDip;

    public double MoveThresholdDip { get; }

    /// <summary>
    /// A pointer moved to <paramref name="current"/>. True EXACTLY ONCE per hover, on the move
    /// that should start the dwell.
    ///
    /// The first sample only anchors, and that is what makes a page appearing under a stationary
    /// pointer harmless: WPF raises a move for the tree change, this takes it as the anchor, and
    /// nothing further happens until the hand actually moves.
    ///
    /// Returns false for every move after the dwell has begun, rather than restarting it. A
    /// restart-on-move rule means a hand creeping toward a tile never opens anything, which is
    /// the opposite of the gesture this serves.
    /// </summary>
    public bool NoteMove(Point current)
    {
        if (_started) return false;

        if (_anchor is not { } anchor)
        {
            _anchor = current;
            return false;
        }

        var dx = current.X - anchor.X;
        var dy = current.Y - anchor.Y;
        if (Math.Abs(dx) < MoveThresholdDip && Math.Abs(dy) < MoveThresholdDip) return false;

        _started = true;
        return true;
    }

    /// <summary>
    /// Forget everything. Called when the pointer leaves, when the page is no longer the one on
    /// screen, and after the open has happened.
    ///
    /// The last of those matters: without it, a shelf that opened, was dismissed, and left the
    /// pointer where it was would never open again, because the intent would still be "started".
    /// </summary>
    public void Reset()
    {
        _anchor = null;
        _started = false;
    }

    /// <summary>Whether the dwell has been started and not yet reset. The view uses this to tell
    /// a running timer from one that was never armed.</summary>
    public bool Started => _started;
}
