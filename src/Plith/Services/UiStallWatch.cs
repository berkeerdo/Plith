using System.Globalization;

namespace Plith.Services;

/// <summary>
/// Watches the UI thread for the thing a person actually sees: the mouse going busy.
///
/// The caller runs <see cref="Tick"/> from a DispatcherTimer at Input priority, and that priority
/// is the whole mechanism. Input sits below Loaded, Render, DataBind and Normal in WPF's queue, so
/// the timer cannot tick while any of them is backed up, and cannot tick at all while the thread is
/// blocked outright. Those two are exactly the conditions that put the busy cursor on screen. The
/// gap between consecutive ticks is therefore not a proxy for the symptom; it is a measurement of
/// it. The same mechanism <see cref="NotchOpenTrace"/> uses, lifted out because it is needed for
/// longer than one open.
///
/// It has NO notion of a window it is watching, and that is the design rather than an omission: it
/// reports every gap over its threshold, for as long as its caller keeps ticking it, and the
/// caller decides how long that is. <see cref="Plith.App"/> ticks it for the length of one launch.
///
/// IT USED TO BE TICKED FOR THE LIFE OF THE PROCESS, and the old reason is kept because it was a
/// good one: the symptom this was built for could not be reproduced on demand, so a probe armed
/// only around a launch or only around a click would be looking away at the moment it is meant to
/// catch. That held until the block was explained. Sections 6 to 8 of docs/PERF-VERIFICATION.md
/// did explain it, and section 3 is the standard the change was then held to: it cut the
/// orchestrator from 33 timer wakeups a second to 2, because wakeups matter on a laptop for
/// reasons CPU per cent does not show, and a probe at 250 ms adds four a second back. Section 6
/// records what those four measured, on both axes, before and after.
///
/// Anchored at construction rather than at the first tick, so a thread that blocks immediately
/// still reports the block. Waiting for a first tick to anchor on would mean the worst case of all,
/// nothing ticking until the hang ends, measured as zero.
/// </summary>
public sealed class UiStallWatch
{
    private readonly Func<long> _clockMs;
    private readonly long _thresholdMs;

    private long _lastTick;
    private long _maxGap;
    private int _ticks;

    /// <summary>The largest gap seen so far. Read on the probe's own tick and passed to
    /// <see cref="StartupTrace.Report"/>, which is a tick later than the settle and deliberately
    /// so: at the settle there is nothing sampled yet.</summary>
    public long MaxGapMs => _maxGap;

    /// <summary>
    /// How many times the probe has fired. Reported alongside the gap wherever the gap is,
    /// because a maximum of zero has two completely different meanings: nothing blocked, or
    /// nothing was ever sampled because the window was shorter than the interval. Without the
    /// count the second reads as the first.
    /// </summary>
    public int Ticks => _ticks;

    /// <param name="thresholdMs">The largest gap considered ordinary. The probe's own interval
    /// plus scheduling slop has to fit under it, or the log fills with the instrument's jitter.</param>
    public UiStallWatch(Func<long> clockMs, long thresholdMs)
    {
        _clockMs = clockMs;
        _thresholdMs = thresholdMs;
        _lastTick = clockMs();
    }

    /// <summary>
    /// A tick of the probe. Returns a line to log when the gap since the previous tick exceeded
    /// the threshold, and null otherwise.
    ///
    /// Reported once rather than on every tick after it: the gap is measured between consecutive
    /// ticks, so by the time it is seen the block has already ended.
    /// </summary>
    public string? Tick()
    {
        var now = _clockMs();
        var gap = now - _lastTick;
        _lastTick = now;
        _ticks++;

        if (gap > _maxGap) _maxGap = gap;

        return gap > _thresholdMs
            ? string.Format(CultureInfo.InvariantCulture, "UI thread stalled {0} ms", gap)
            : null;
    }
}
