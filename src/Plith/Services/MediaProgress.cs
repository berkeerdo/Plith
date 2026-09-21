namespace Plith.Services;

/// <summary>
/// Where a track has got to, and how to write that as a clock.
///
/// Pure, and that is the reason it is a class of its own rather than two private methods on the
/// widget. The suite is not STA, so a UserControl cannot be constructed in a test at all: pure
/// code is the only part of the media page a test can reach. See NotchEventPolicy for the same
/// move made for the same reason.
///
/// SMTC reports a position with a timestamp rather than a live value. Asking for it every second
/// would be a polling loop against a cross-process source; interpolating from the stamp is both
/// cheaper and what makes a source that only reports on seek still look right.
/// </summary>
public static class MediaProgress
{
    /// <summary>
    /// The position now, given a reading taken at <paramref name="lastUpdated"/>.
    ///
    /// Total on purpose: every degenerate input a real source has been seen to produce returns
    /// something drawable rather than throwing. A duration of zero returns zero because the
    /// caller draws no bar in that case, and a stamp in the future returns the reading itself
    /// rather than running the bar backwards.
    /// </summary>
    public static TimeSpan Elapsed(TimeSpan position, DateTimeOffset lastUpdated, TimeSpan duration,
                                   bool isPlaying, DateTimeOffset now)
    {
        if (duration <= TimeSpan.Zero) return TimeSpan.Zero;

        var elapsed = position;
        if (isPlaying)
        {
            var age = now - lastUpdated;
            if (age > TimeSpan.Zero) elapsed += age;
        }

        if (elapsed < TimeSpan.Zero) return TimeSpan.Zero;
        return elapsed > duration ? duration : elapsed;
    }

    /// <summary>
    /// A fraction of the track, back into a position to seek to.
    ///
    /// The inverse of what the bar draws, and total for the same reason <see cref="Elapsed"/> is:
    /// a Slider whose Maximum is momentarily zero divides to NaN, and NaN reaching
    /// TimeSpan.FromTicks throws inside a mouse handler. Seeking to the start is wrong in a way a
    /// person can see and undo; a crash in a media page is not.
    ///
    /// Rounded to whole seconds, because that is the resolution the two clocks beside the bar
    /// show: a write a person can repeat exactly is worth more here than sub-second precision
    /// nobody can aim at on a 232 DIP bar.
    /// </summary>
    public static TimeSpan PositionFor(double fraction, TimeSpan duration)
    {
        if (duration <= TimeSpan.Zero || double.IsNaN(fraction)) return TimeSpan.Zero;

        var clamped = Math.Clamp(fraction, 0, 1);
        return TimeSpan.FromSeconds(Math.Round(duration.TotalSeconds * clamped));
    }

    /// <summary>
    /// A track clock: minutes and padded seconds, with hours only when there are any.
    ///
    /// Not a format string on the call site, because the remaining time is written as the
    /// negative of this and a span that has gone slightly negative would print a second minus
    /// sign. Clamped here instead, once.
    /// </summary>
    public static string Clock(TimeSpan span)
    {
        if (span < TimeSpan.Zero) span = TimeSpan.Zero;

        return span.TotalHours >= 1
            ? $"{(int)span.TotalHours}:{span.Minutes:00}:{span.Seconds:00}"
            : $"{(int)span.TotalMinutes}:{span.Seconds:00}";
    }
}
