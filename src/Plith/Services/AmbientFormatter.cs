using System.Globalization;

namespace Plith.Services;

/// <summary>
/// Every display decision the ambient row makes, with nothing that touches the world.
///
/// This is the same gather/decide split as FullscreenVideoWatcher / FullscreenVideoDetector,
/// and for the same reason: the headless suite is not STA and cannot construct a card's view,
/// so anything that must be covered has to be reachable without one. The card reads the clock
/// and the power status; this file decides what those become on screen.
/// </summary>
public static class AmbientFormatter
{
    /// <summary>
    /// The clock column: a short time and a year-less date.
    ///
    /// ShortTimePattern with the seconds field stripped rather than a hardcoded "HH:mm",
    /// so a 12-hour culture gets a 12-hour clock. The row is only a couple of lines tall,
    /// so the date drops the year — a clock that is showing the current minute has already
    /// established which year it is.
    /// </summary>
    public static (string Time, string Date) FormatClock(DateTime now, CultureInfo culture)
    {
        var pattern = culture.DateTimeFormat.ShortTimePattern.Replace(":ss", string.Empty);
        var time = now.ToString(pattern, culture);
        var date = now.ToString(culture.DateTimeFormat.MonthDayPattern, culture);
        return (time, date);
    }
}
