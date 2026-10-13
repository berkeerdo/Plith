using System.Globalization;

namespace Plith.Services;

/// <summary>
/// Times one page turn of the open notch's widget frame, frame by frame, and separates a slide
/// that stuttered from a slide that was merely short of anything to look at.
///
/// This exists because of a report with no instrument behind it, carried as the open question in
/// docs/ROADMAP.md section 9: the turn onto the shelf page comes in choppy while the other four
/// do not. What had already been measured was DURATION — about 340 ms end to end, in
/// docs/PERF-VERIFICATION.md section 4 — and duration cannot answer it. A 260 ms slide that
/// delivers every frame and a 260 ms slide that delivers half of them take exactly as long.
///
/// TWO AXES, because three different defects produce the one symptom and a single axis cannot
/// tell them apart:
///
///   TIME   intervals between composition ticks. A slide starved of the UI thread drops frames,
///          and the gaps are where that shows. <c>median</c> is the baseline the other columns
///          are read against, <c>worst</c> the largest single gap, and <c>late</c> how many
///          gaps reached 1.5x the median — one missed refresh at 60 Hz. The count is reported
///          beside the worst case because one hitch and five hitches in the same slide are
///          different defects that the same worst case describes.
///   VALUE  the animated position at each of those ticks. A frame that arrived on time carrying
///          the position of the one before it is a frame a person sees as a pause, and every
///          column on the time axis calls that turn perfect. <c>still</c> counts them. This
///          axis is not hypothetical here: the slide is <see cref="Views.Presentation.NotchGeometry.PageSlideDip"/>
///          DIP under a cubic ease-out, so its last frames move a fraction of a pixel, and
///          quantization would appear here and nowhere else.
///
/// What it CANNOT see, stated because the same limit has been claimed away twice in this repo's
/// history. Every stamp is taken on the UI thread, so nothing here sees the frame the monitor
/// scanned out; this measures what WPF computed and handed to the compositor. It can prove
/// starvation and it can prove a value that failed to advance. A clean line on both axes is
/// therefore not a verdict of "no defect": it rules out this process and leaves the third
/// hypothesis, that the page sliding in simply draws nothing (docs/SHELF-VERIFICATION.md
/// section 10.29), which only a capture can answer.
///
/// Pure, with the clock injected, for the reason <see cref="NotchPager"/> gives: the suite is not
/// STA, so a type that touched a <c>UserControl</c> could not be tested at all. Nothing here
/// reads a timer or a dependency property of its own — the caller samples and feeds in numbers.
///
/// MICROSECONDS, not milliseconds, and the difference decides whether this works at all.
/// <c>Environment.TickCount64</c> advances in steps of about 15.6 ms, which is one frame at
/// 60 Hz: an instrument built on it would quantize every interval it set out to measure and
/// report a perfectly smooth slide as wildly uneven, or the reverse. The app hands this a clock
/// derived from <c>Stopwatch</c>; the tests hand it whatever they please.
/// </summary>
public sealed class PageTurnTrace
{
    /// <summary>
    /// How much longer than the median a frame has to take before it counts as late.
    ///
    /// 1.5x is one missed refresh: at 60 Hz it means 25 ms where 16.6 was due. Expressed against
    /// the median rather than against a fixed 16.6 ms because the refresh rate is the monitor's
    /// business, and a 144 Hz display would otherwise report every frame as perfect.
    /// </summary>
    private const double LateFactor = 1.5;

    private readonly Func<long> _clockUs;

    private readonly List<long> _intervalsUs = [];
    private readonly List<double> _values = [];

    private long _lastStampUs;
    private bool _inFlight;
    private int _completed;

    private int _from;
    private int _to;
    private int _pageCount;
    private int _durationMs;

    public PageTurnTrace(Func<long> clockUs) => _clockUs = clockUs;

    /// <summary>
    /// A page turn began sliding.
    ///
    /// Resets everything rather than appending, and that discards a turn still in flight on
    /// purpose: a slide can be replaced by the next one before its window is out, because
    /// <c>ShowCurrentPage</c> removes an outgoing page the moment a new turn starts. The samples
    /// of a turn that was cut off belong to neither line, and carrying them into the next one
    /// would report a step the next slide never made.
    /// </summary>
    public void Begin(int fromIndex, int toIndex, int pageCount, int durationMs)
    {
        _from = fromIndex;
        _to = toIndex;
        _pageCount = pageCount;
        _durationMs = durationMs;

        _intervalsUs.Clear();
        _values.Clear();
        _lastStampUs = _clockUs();
        _inFlight = true;
    }

    /// <summary>
    /// One composition tick, carrying the animated position of the page that is sliding in.
    ///
    /// The interval is measured from <see cref="Begin"/> for the first frame rather than from the
    /// frame before it. A slide whose FIRST frame arrives late is the starved case this whole
    /// instrument exists to find, and measuring only between frames cannot see it: there is no
    /// earlier frame to measure the gap from. <see cref="NotchOpenTrace.Tick"/> anchors its stall
    /// at the click for the same reason.
    ///
    /// Being called with nothing in flight is routine rather than an error. The caller detaches
    /// its <c>Rendering</c> handler a moment after the settle, so a frame can still land in
    /// between.
    /// </summary>
    public void Frame(double value)
    {
        if (!_inFlight) return;

        var now = _clockUs();
        _intervalsUs.Add(now - _lastStampUs);
        _lastStampUs = now;
        _values.Add(value);
    }

    /// <summary>
    /// The turn's window is out. Returns the line to log, or null when there is nothing to
    /// report.
    ///
    /// The ordinal is assigned here rather than at <see cref="Begin"/>, so it counts turns that
    /// completed. The comparison this instrument is built for sets the turn onto the shelf beside
    /// the ordinary ones in the same run, and a reader matching log lines to gestures needs the
    /// count to mean what it says.
    /// </summary>
    public string? Settled()
    {
        if (!_inFlight) return null;
        _inFlight = false;
        _completed++;

        var header = string.Format(CultureInfo.InvariantCulture, "Turn #{0}: {1}->{2} of {3} in {4}ms: ",
                                   _completed, _from, _to, _pageCount, _durationMs);

        // A turn that produced no composition tick is the worst outcome this instrument can
        // find, and it must not be formatted like the others: "median 0.0ms, 0 late" reads
        // exactly like a perfect slide.
        if (_intervalsUs.Count == 0) return header + "no frames rendered";

        var median = Median(_intervalsUs);
        var late = _intervalsUs.Count(us => us >= median * LateFactor);

        var travel = Math.Abs(_values[^1] - _values[0]);
        var biggestStep = 0.0;
        var still = 0;
        for (var i = 1; i < _values.Count; i++)
        {
            var step = Math.Abs(_values[i] - _values[i - 1]);
            if (step > biggestStep) biggestStep = step;
            if (_values[i] == _values[i - 1]) still++;
        }

        return header + string.Format(CultureInfo.InvariantCulture,
            "{0} frames, median {1:0.0}ms, worst {2:0.0}ms, {3} late; " +
            "travel {4:0.00} DIP, biggest step {5:0.00}, {6} still",
            _intervalsUs.Count, median / 1000.0, _intervalsUs.Max() / 1000.0, late,
            travel, biggestStep, still);
    }

    /// <summary>
    /// The median interval, not the mean, and the choice is what keeps the other columns honest.
    ///
    /// The median is the baseline <c>late</c> is counted against, so it has to describe the
    /// frames that arrived normally. One 200 ms stall inside a 260 ms slide drags a mean far
    /// enough that the stall itself no longer reaches 1.5x it, and the instrument would report a
    /// slide with one severe hitch as entirely ordinary.
    /// </summary>
    private static double Median(List<long> samples)
    {
        var sorted = samples.Order().ToArray();
        var mid = sorted.Length / 2;
        return sorted.Length % 2 == 1 ? sorted[mid] : (sorted[mid - 1] + sorted[mid]) / 2.0;
    }
}
