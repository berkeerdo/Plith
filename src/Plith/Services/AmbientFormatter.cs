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

    /// <summary>
    /// The clock, with the AM/PM designator handed back separately.
    ///
    /// The notch's clock page draws the digits at 46 point, and on a 12-hour machine the
    /// designator at that size runs straight into the weather mark beside it. MEASURED, in
    /// widget-clock-12hour.png, which is a render that did not exist until someone asked whether
    /// an AM/PM machine worked: the format was right and the layout was not.
    ///
    /// So the page draws "12:49" large and "PM" small, which is what Windows' own clock does and
    /// what the space allows. <see cref="FormatClock"/> is unchanged and still returns one
    /// string, because the ambient row has room for it and two surfaces asking the same question
    /// two ways is how they start disagreeing.
    ///
    /// Designator is empty on a 24-hour culture, so a caller can collapse its own element without
    /// asking what kind of clock this is.
    ///
    /// KNOWN LIMIT: a culture that puts its designator BEFORE the time still gets it after. Such
    /// cultures exist and none of them is this product's user; the alternative is carrying the
    /// position as well and laying the page out twice, for a case nothing has ever asked for.
    /// </summary>
    public static (string Digits, string Designator) FormatClockParts(
        DateTime now, CultureInfo culture)
    {
        var pattern = culture.DateTimeFormat.ShortTimePattern.Replace(":ss", string.Empty);

        // Whether this culture asks for a designator at all is a property of its PATTERN, not of
        // whether the strings exist: every culture has an AMDesignator, and most 24-hour ones
        // leave it blank while some do not.
        var wants = pattern.Contains('t');
        if (!wants) return (now.ToString(pattern, culture), string.Empty);

        // The designator's own token removed, then trimmed: "h:mm tt" leaves a trailing space
        // that would show up as a gap before nothing.
        var digitsPattern = pattern.Replace("tt", string.Empty).Replace("t", string.Empty).Trim();
        var designator = now.Hour < 12
            ? culture.DateTimeFormat.AMDesignator
            : culture.DateTimeFormat.PMDesignator;

        return (now.ToString(digitsPattern, culture), designator);
    }

    /// <summary>
    /// The weekday, in the culture's own words.
    ///
    /// Here rather than inline in the widget, for the reason the clock above is here: two
    /// surfaces showing the same fact must not write it two ways. It is a separate method rather
    /// than a third member of FormatClock's tuple because only the notch's clock page has the
    /// room for it, and adding it to the tuple would change every existing caller to ignore it.
    /// </summary>
    public static string FormatWeekday(DateTime now, CultureInfo culture)
        => culture.DateTimeFormat.GetDayName(now.DayOfWeek);

    /// <summary>
    /// The battery column. Returns Show=false for every case that has no honest percentage:
    /// a desktop (BatteryFlag bit 128), an unknown level (255), or a failed read (null).
    /// All three collapse the column rather than rendering a placeholder, because a battery
    /// readout that might be wrong is worse than none on a row this small.
    /// </summary>
    public static (bool Show, string Text, bool Charging) FormatBattery(BatteryStatusRaw? raw)
    {
        const byte NoSystemBattery = 128;
        const byte UnknownPercent = 255;
        const byte OnAcPower = 1;

        if (raw is not { } s) return (false, string.Empty, false);
        if ((s.BatteryFlag & NoSystemBattery) != 0) return (false, string.Empty, false);
        if (s.BatteryLifePercent == UnknownPercent) return (false, string.Empty, false);

        // ACLineStatus is 0 offline, 1 online, 255 unknown. Only an explicit 1 counts as
        // charging: guessing on 255 puts a charging glyph on a laptop that is discharging.
        return (true, $"{s.BatteryLifePercent}%", s.ACLineStatus == OnAcPower);
    }
}
