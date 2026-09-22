using System.Globalization;

namespace Plith.Services;

/// <summary>
/// Times one open of the notch's widget frame, and separates the wait a person actually felt
/// from the animation they were supposed to see.
///
/// This exists because of a report that had no instrument behind it: the mouse going busy for
/// about a second on first use. A busy cursor is the UI thread not pumping input, which is NOT
/// the same thing as "time until the panel is open". The expansion is a deliberate 340 ms
/// quintic and is not a hang. Timing the open alone would have called that 340 ms a defect and
/// missed a block underneath it, so the two are measured separately here.
///
/// Four consecutive spans, which PARTITION the open rather than overlapping it. Adding them up
/// lands on the total, so no column can be blamed twice:
///
///   layout  click to the first layout pass after the panel switched to the widget frame. This
///           is where the pages are first measured and arranged, because WidgetHost is
///           Collapsed until an open and a collapsed element is never laid out. Reaching it
///           before the deferral is not an accident of timing: OnNotchClicked switches the
///           panel synchronously, and WPF runs layout at Render priority, which is ABOVE the
///           Loaded priority the rest of the open is deferred to.
///   defer   the remainder of the dispatcher turn OnNotchClicked waits on purpose, so the
///           ambient row exists before the expansion animates. Reported apart from the layout
///           precisely so it is never mistaken for it; it is meant to be there and to be short.
///   show    policy, pager reset, reposition and the animation start, all synchronous.
///   settle  the show returning to the UI thread going idle. Posted at ContextIdle, which WPF
///           runs only once everything above it has drained, so this is the span where work
///           queued by the open but not done inside it would appear.
///
/// The stall is the separate half, and the only figure that measures the reported symptom. See
/// Tick.
///
/// Wall-clock only, and deliberately: the caller supplies the clock, so the tests drive it and
/// the app hands it a monotonic one. Nothing here reads a timer of its own.
///
/// GPU and DWM compositing are outside all of it. Every stamp is taken on the UI thread, so
/// nothing here can see the frame the monitor actually scanned out.
///
/// An earlier version of this type timed "the first frame rendered after the show began", hung
/// off CompositionTarget.Rendering. It was measured on hardware and it was worthless: that
/// event fires per frame whether or not the content in question was laid out, and the notch is
/// animating by then, so it reported 0 ms on every open including the first. The lesson is the
/// one this repo keeps paying for, and it applies to instruments as readily as to features.
/// The tests were green the whole time.
/// </summary>
public sealed class NotchOpenTrace
{
    private readonly Func<long> _clockMs;

    private long _click;
    private long _laidOut;
    private long _deferred;
    private long _shown;

    private bool _sawLayout;

    private long _lastTick;
    private long _maxStall;
    private int _ticks;

    /// <summary>
    /// Whether a click is still waiting to settle.
    ///
    /// Not bookkeeping for its own sake: every method below is reachable when no open is
    /// happening. LayoutUpdated, the probe timer and the settle callback all outlive the moment
    /// they were armed for, so being called with nothing in flight is routine rather than an
    /// error.
    /// </summary>
    private bool _inFlight;

    private int _completed;

    public NotchOpenTrace(Func<long> clockMs) => _clockMs = clockMs;

    /// <summary>
    /// A click committed to opening the frame.
    ///
    /// Called after OnNotchClicked's guards rather than at the top of it, so a click the notch
    /// declines is not counted as an open that took forever to arrive.
    ///
    /// Resets everything rather than only the click stamp. An attempt can be abandoned between
    /// here and the settle (the deferred callback bails on edit mode, on a presentation change
    /// or on a closed home state), and the next click must not inherit its clock or its stall.
    /// </summary>
    public void Click()
    {
        _click = _clockMs();
        _laidOut = _click;
        _deferred = _click;
        _shown = _click;
        _sawLayout = false;
        _lastTick = _click;
        _maxStall = 0;
        _ticks = 0;
        _inFlight = true;
    }

    /// <summary>
    /// A layout pass completed while this open was in flight.
    ///
    /// Only the first counts. The expansion animates the panel's shape, so later passes are
    /// guaranteed and would otherwise overwrite the stamp with a time that has nothing to do
    /// with laying the pages out.
    ///
    /// An open with no pass at all is normal rather than a failure, and reads as zero: the
    /// pages are laid out once and stay laid out, so every open after the first can legitimately
    /// produce none.
    /// </summary>
    public void LaidOut()
    {
        if (!_inFlight || _sawLayout) return;
        _sawLayout = true;
        _laidOut = _clockMs();
    }

    /// <summary>The deferred callback got control, one dispatcher turn after the click.</summary>
    public void Deferred() => _deferred = _clockMs();

    /// <summary>ShowOsd returned: the expansion has been started.</summary>
    public void Shown() => _shown = _clockMs();

    /// <summary>
    /// A tick of the stall probe.
    ///
    /// The caller runs this on a DispatcherTimer at Input priority, and that priority is the
    /// whole mechanism. Input sits below Loaded, Render, DataBind and Normal in WPF's queue, so
    /// this timer cannot tick while any of them is backed up, and cannot tick at all while the
    /// thread is blocked outright. Those two are exactly the conditions that put the busy cursor
    /// on screen. So the largest gap between consecutive ticks is not a proxy for the symptom;
    /// it is a measurement of it.
    ///
    /// Anchored at the click rather than at the first tick, so a thread that blocks immediately
    /// still reports the block. Waiting for a first tick to anchor on would mean the worst case
    /// of all, nothing ticking until the hang ends, measured as zero.
    ///
    /// The count is reported alongside the stall because a stall of zero has two completely
    /// different meanings: nothing blocked, or nothing was ever sampled because the open was
    /// shorter than the probe interval. Without the count the second reads as the first, and
    /// the instrument quietly reports "no block" for a window it never looked at.
    /// </summary>
    public void Tick()
    {
        if (!_inFlight) return;

        var now = _clockMs();
        var gap = now - _lastTick;
        if (gap > _maxStall) _maxStall = gap;
        _lastTick = now;
        _ticks++;
    }

    /// <summary>
    /// The UI thread went idle after the show. Returns the line to log, or null when there is
    /// nothing to report.
    ///
    /// The ordinal is assigned here rather than at the click, so it counts opens that actually
    /// completed. An abandoned attempt takes no number, which matters because the entire
    /// question is what the FIRST open costs: an attempt that silently consumed #1 would leave
    /// the real first open reporting itself as the second.
    /// </summary>
    public string? Settled()
    {
        if (!_inFlight) return null;
        _inFlight = false;

        var now = _clockMs();
        _completed++;

        return string.Format(CultureInfo.InvariantCulture,
            "Open #{0}: {1}ms total (layout {2}, defer {3}, show {4}, settle {5}), max UI stall {6}ms over {7} ticks",
            _completed, now - _click, _laidOut - _click, _deferred - _laidOut, _shown - _deferred,
            now - _shown, _maxStall, _ticks);
    }
}
