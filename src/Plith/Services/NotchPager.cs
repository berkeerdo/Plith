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
    /// How small an incoming delta has to be before the pager will accept another page turn.
    ///
    /// A touchpad does not stop cleanly: after the fingers lift, inertia keeps sending deltas
    /// that decay towards zero. Those small tail deltas are the only signal that the swipe is
    /// over, so they rearm rather than being discarded — and anything still large is treated as
    /// the same swipe continuing.
    ///
    /// Provisional, for the same reason as <see cref="CommitThreshold"/>.
    /// </summary>
    public const int RearmFloor = 40;

    /// <summary>
    /// How long the wheel has to go quiet before the next delta may page again.
    ///
    /// The rearm floor alone is not enough, and assuming it was is a real defect this constant
    /// exists to close: a mouse's tilt wheel sends exactly one WHEEL_DELTA per detent and never
    /// anything smaller, so a pager rearmed only by small deltas would page once and then stay
    /// deaf forever. A touchpad rearms either way — its inertia decays through the floor — but
    /// the tilt wheel only ever rearms on the gap between detents.
    ///
    /// 150 ms is short enough that deliberate repeated detents each page, and long enough that
    /// the deltas inside one swipe do not.
    /// </summary>
    public const int IdleRearmMs = 150;

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
            // Still inside a swipe that has already paged. Only a delta small enough to be the
            // decaying tail rearms; anything larger is more of the same gesture.
            if (Math.Abs(delta) < RearmFloor) Rest();
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
    public void Reset()
    {
        Index = 0;
        RestAndForgetTiming();
    }

    private void Step(int direction) => Index = Wrap(Index + direction);

    private int Wrap(int index) => ((index % PageCount) + PageCount) % PageCount;
}
