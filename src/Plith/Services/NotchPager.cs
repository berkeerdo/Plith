namespace Plith.Services;

/// <summary>
/// Which widget page the open notch is showing, and how a stream of wheel deltas turns into
/// page turns.
///
/// Pure by design. The suite is not STA, so nothing that touches a <c>UserControl</c> can be
/// tested at all — and paging is the part of the widget frame most worth testing, because the
/// input it consumes is not a page count. One physical two-finger swipe emits a stream of small
/// deltas, and a naive "one delta, one page" would fly through every widget before the fingers
/// left the touchpad. So deltas accumulate, one page commits per threshold, and nothing else
/// commits until the accumulator falls back near zero.
///
/// The index lives here rather than in the frame for the reason recorded in the spec's §7:
/// every defect on this branch came from state derived once, in a callback that turned out not
/// to fire. An index owned by an object with no animation, no timer and no completion handler
/// cannot go stale.
/// </summary>
public sealed class NotchPager
{
    /// <summary>
    /// One <c>WHEEL_DELTA</c>. A mouse's tilt wheel sends exactly this per detent, so a tilt
    /// pages once per click, which is the behaviour a mouse user expects.
    ///
    /// Provisional until measured on the user's own touchpad: driver delta magnitudes differ,
    /// and this cannot be reasoned about from here.
    /// </summary>
    public const int CommitThreshold = 120;

    /// <summary>
    /// DELETED, and the note is left in its place because the constant was wrong in a way that
    /// took a user's report to see.
    ///
    /// It was 40: a delta smaller than that was treated as the decaying tail of a swipe and
    /// rearmed the pager. The reasoning assumed a touchpad sends LARGE deltas while the fingers
    /// move and small ones after they lift.
    ///
    /// Measured on this project's own user, 2026-09-21: their touchpad sends deltas of one to
    /// six for the WHOLE gesture. Every delta was therefore below the floor, every one rearmed
    /// the pager, and a single flick paged again as soon as its stream summed to another 120. On
    /// Plith's own pages that showed up as flying past a page or two; on the shelf page it closed
    /// a window, and what a person saw was "I get to the shelf and it closes by itself". Their
    /// log, with the arrival at delta=-2 and the departure 1.2 seconds later at delta=+3, is in
    /// docs/SHELF-VERIFICATION.md section 10.19.
    ///
    /// The idle gap below is the signal that survives contact with real hardware: silence means
    /// the gesture is over, whatever the magnitudes were.
    /// </summary>
    private const string RearmFloorRemoved = "see IdleRearmMs";

    /// <summary>
    /// How long the wheel has to go quiet before the next delta may page again.
    ///
    /// THE ONLY WAY THE PAGER REARMS, since the rearm floor was deleted (see above). Silence is
    /// the one signal that means "the gesture is over" on every device tested: a tilt wheel's
    /// detents are separated by it, a touchpad's stream ends with it, and a mouse wheel's notches
    /// have it whenever a person is not spinning the wheel continuously.
    ///
    /// ONE GESTURE, ONE PAGE is the rule that falls out of it, and that is the intended feel
    /// rather than a side effect: a flick moves one page and stops, so the page a person aimed at
    /// is the page they get. Paging three along takes three flicks.
    ///
    /// 120 rather than the 150 it was. With the floor gone this is the only rearm, so it decides
    /// how soon a second deliberate flick counts, and a touchpad's stream has messages 8 to 30 ms
    /// apart, which 120 clears several times over.
    /// </summary>
    public const int IdleRearmMs = 120;

    private int _accumulated;
    private bool _armed = true;
    private long _lastTimestampMs;
    private bool _hasSeenDelta;

    public NotchPager(int pageCount)
    {
        ArgumentOutOfRangeException.ThrowIfLessThan(pageCount, 1);
        PageCount = pageCount;
    }

    public int PageCount { get; private set; }

    public int Index { get; private set; }

    /// <summary>
    /// Feed one wheel delta. Returns true if this delta turned a page.
    ///
    /// Positive is "towards the next page". The caller owns the sign convention for each
    /// message — <c>WM_MOUSEHWHEEL</c> is positive-right while <c>WM_MOUSEWHEEL</c> is
    /// positive-up — because that is a Win32 detail, not a paging one.
    /// </summary>
    public bool Accumulate(int delta, long timestampMs)
    {
        // One page is not a carousel. Swallowing here rather than at the call site keeps the
        // caller from having to know, and keeps a single-widget notch from feeling broken.
        if (PageCount < 2) return false;

        // The gap is measured before anything else, so a quiet wheel rearms even when the delta
        // that broke the silence is a full detent. Time comes in as a parameter rather than
        // being read here: the whole point of this type is that it can be tested.
        var gap = _hasSeenDelta ? timestampMs - _lastTimestampMs : long.MaxValue;
        _lastTimestampMs = timestampMs;
        _hasSeenDelta = true;
        if (gap >= IdleRearmMs) Rest();

        if (!_armed)
        {
            // Still inside a gesture that has already paged, and nothing here can end it: only
            // silence can, which the gap check above applies. The line that used to sit here
            // rearmed on any delta below 40, which on a touchpad whose deltas are all below 40
            // meant every message rearmed and one flick paged over and over. See RearmFloor's
            // own note for the measurement.
            return false;
        }

        _accumulated += delta;
        if (Math.Abs(_accumulated) < CommitThreshold) return false;

        Step(Math.Sign(_accumulated));
        _accumulated = 0;
        _armed = false;
        return true;
    }

    /// <summary>Declare the gesture over, so the next delta may page again. Called by the
    /// caller when input goes quiet, and by <see cref="GoTo"/>.</summary>
    public void Rest()
    {
        _accumulated = 0;
        _armed = true;
    }

    /// <summary>Forget the last gesture's timing as well as its accumulator. Called when the
    /// notch closes, so a swipe minutes later is never measured against it.</summary>
    public void RestAndForgetTiming()
    {
        Rest();
        _hasSeenDelta = false;
    }

    /// <summary>
    /// Page directly, as a page-dot click does. Out-of-range values wrap rather than throwing,
    /// so a caller need not know the count.
    ///
    /// Rearms: a dot clicked while inertia was still arriving must leave the wheel usable,
    /// otherwise the pager stays deaf until the next quiet moment.
    /// </summary>
    public bool GoTo(int index)
    {
        var target = Wrap(index);
        var moved = target != Index;
        Index = target;
        Rest();
        return moved;
    }

    /// <summary>
    /// Change how many pages exist, keeping the index inside the new range.
    ///
    /// Pages come and go: the media page only exists while something is playing, and the
    /// battery column is absent on a desktop. Shrinking the count under a high index must not
    /// leave the pager pointing at a page that is no longer there.
    /// </summary>
    public void SetPageCount(int pageCount)
    {
        ArgumentOutOfRangeException.ThrowIfLessThan(pageCount, 1);
        PageCount = pageCount;
        if (Index >= PageCount) Index = PageCount - 1;
    }

    /// <summary>Return to the first page. Used when the notch closes, so the next open starts
    /// where the last one did not leave off — remembering the page is deferred, per the spec.</summary>
    public void Reset() => ResetTo(0);

    /// <summary>
    /// Open on a given page, clamping into range.
    ///
    /// Clamps rather than wrapping, which is what <see cref="GoTo"/> does: a dot clicked past the
    /// end means the other end, but an opening index past the end is a bug upstream and wrapping
    /// would hide it by opening somewhere plausible.
    ///
    /// Forgets the last gesture's timing as well as its accumulator, so a swipe long after the
    /// notch reopened is never measured against the one that closed it.
    /// </summary>
    public void ResetTo(int index)
    {
        Index = Math.Clamp(index, 0, PageCount - 1);
        RestAndForgetTiming();
    }

    private void Step(int direction) => Index = Wrap(Index + direction);

    private int Wrap(int index) => ((index % PageCount) + PageCount) % PageCount;
}
