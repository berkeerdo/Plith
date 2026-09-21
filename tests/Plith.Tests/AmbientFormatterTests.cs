using System.Globalization;
using Plith.Services;

namespace Plith.Tests;

public class AmbientFormatterTests
{
    // Explicit cultures on every case: the ambient row shows a time, and a test that reads
    // the machine's locale passes on the author's box and fails in CI or on a Turkish
    // install — which is this project's actual default.
    private static readonly CultureInfo Tr = new("tr-TR");
    private static readonly CultureInfo EnUs = new("en-US");

    [Fact]
    public void FormatClock_UsesA24HourClockInACultureThatWants24Hour()
    {
        var (time, _) = AmbientFormatter.FormatClock(new DateTime(2026, 9, 7, 14, 5, 0), Tr);
        Assert.Equal("14:05", time);
    }

    [Fact]
    public void FormatClock_UsesA12HourClockInACultureThatWants12Hour()
    {
        var (time, _) = AmbientFormatter.FormatClock(new DateTime(2026, 9, 7, 14, 5, 0), EnUs);
        Assert.Contains("2:05", time);

        // AND the designator, which this test used to leave unasserted: "2:05" with no PM would
        // have passed it, and on a 12-hour machine that is an afternoon clock claiming to be a
        // morning one. The pattern this method edits is the culture's ShortTimePattern with any
        // seconds removed, so the designator surviving that edit is the thing worth checking.
        Assert.Contains(EnUs.DateTimeFormat.PMDesignator, time);
    }

    [Fact]
    public void FormatClock_KeepsTheAmDesignatorToo()
    {
        var (time, _) = AmbientFormatter.FormatClock(new DateTime(2026, 9, 7, 9, 5, 0), EnUs);

        Assert.Contains("9:05", time);
        Assert.Contains(EnUs.DateTimeFormat.AMDesignator, time);
    }

    [Fact]
    public void FormatClock_DropsSecondsWithoutEatingTheDesignator()
    {
        // A culture whose short time pattern carries seconds is the case the Replace(":ss", "")
        // was written for, and the one where an over-eager edit could take the designator with
        // it. Built here rather than hunted for in the installed culture list, which differs
        // between machines and would make this test's subject depend on the box it runs on.
        var seconds = (CultureInfo)EnUs.Clone();
        seconds.DateTimeFormat.ShortTimePattern = "h:mm:ss tt";

        var (time, _) = AmbientFormatter.FormatClock(new DateTime(2026, 9, 7, 14, 5, 9), seconds);

        Assert.Equal("2:05 PM", time);
    }

    [Fact]
    public void FormatClock_DateIsDayAndMonthWithoutTheYear()
    {
        // The row is a few DIP tall. A four-digit year buys nothing next to a clock that
        // already says it is today.
        var (_, date) = AmbientFormatter.FormatClock(new DateTime(2026, 9, 7, 14, 5, 0), EnUs);
        Assert.DoesNotContain("2026", date);
        Assert.Contains("7", date);
    }

    [Fact]
    public void FormatClock_HasNoSeconds()
    {
        // The card ticks at 1 Hz so the minute rolls over promptly, not so seconds render.
        var (time, _) = AmbientFormatter.FormatClock(new DateTime(2026, 9, 7, 14, 5, 33), Tr);
        Assert.Equal("14:05", time);
    }

    // GetSystemPowerStatus's own sentinel values, from the Win32 documentation. Named rather
    // than inlined because 128 and 255 are unreadable at the call site and mean two very
    // different things.
    private const byte NoSystemBattery = 128;
    private const byte UnknownPercent = 255;
    private const byte OnAcPower = 1;
    private const byte OnBattery = 0;

    [Fact]
    public void FormatBattery_HidesTheColumnOnADesktop()
    {
        // BatteryFlag bit 128 is "no system battery". A desktop must show no battery column
        // at all rather than a zero or a dash — and this machine cannot be both a desktop and
        // a laptop, so the decision has to be reachable without one.
        var (show, _, _) = AmbientFormatter.FormatBattery(
            new BatteryStatusRaw(NoSystemBattery, UnknownPercent, OnAcPower));
        Assert.False(show);
    }

    [Fact]
    public void FormatBattery_HidesTheColumnWhenThePercentIsUnknown()
    {
        var (show, _, _) = AmbientFormatter.FormatBattery(new BatteryStatusRaw(0, UnknownPercent, OnBattery));
        Assert.False(show);
    }

    [Fact]
    public void FormatBattery_HidesTheColumnWhenTheCallFailed()
    {
        var (show, _, _) = AmbientFormatter.FormatBattery(null);
        Assert.False(show);
    }

    [Fact]
    public void FormatBattery_ShowsAWholePercent()
    {
        var (show, text, charging) = AmbientFormatter.FormatBattery(new BatteryStatusRaw(0, 73, OnBattery));
        Assert.True(show);
        Assert.Equal("73%", text);
        Assert.False(charging);
    }

    [Fact]
    public void FormatBattery_ReportsChargingFromTheAcLineStatus()
    {
        var (_, _, charging) = AmbientFormatter.FormatBattery(new BatteryStatusRaw(0, 73, OnAcPower));
        Assert.True(charging);
    }

    [Fact]
    public void FormatBattery_TreatsAnUnknownAcLineStatusAsNotCharging()
    {
        // ACLineStatus 255 means "unknown". Guessing "charging" there would show a charging
        // glyph on a laptop that is discharging, which is worse than showing none.
        var (_, _, charging) = AmbientFormatter.FormatBattery(new BatteryStatusRaw(0, 73, 255));
        Assert.False(charging);
    }

    [Fact]
    public void FormatWeekday_UsesTheCulturesOwnWord()
    {
        var sunday = new DateTime(2026, 9, 20);

        Assert.Equal("Sunday", AmbientFormatter.FormatWeekday(sunday, new CultureInfo("en-GB")));
        Assert.Equal("Pazar", AmbientFormatter.FormatWeekday(sunday, new CultureInfo("tr-TR")));
    }

    [Fact]
    public void FormatWeekday_CoversEveryDay()
    {
        // Seven distinct words, which is the only property worth asserting without hard-coding
        // a calendar: the day comes from DayOfWeek and the word from the culture.
        var culture = new CultureInfo("en-GB");
        var start = new DateTime(2026, 9, 21);
        var names = Enumerable.Range(0, 7)
            .Select(i => AmbientFormatter.FormatWeekday(start.AddDays(i), culture))
            .ToList();

        Assert.Equal(7, names.Distinct().Count());
        Assert.Equal("Monday", names[0]);
    }

    // --- FormatClockParts: the designator, separately, so the page can size it ---------------

    [Fact]
    public void FormatClockParts_SplitsTheDesignatorOffOnA12HourCulture()
    {
        var (digits, meridiem) = AmbientFormatter.FormatClockParts(
            new DateTime(2026, 9, 7, 14, 5, 0), EnUs);

        Assert.Equal("2:05", digits);
        Assert.Equal("PM", meridiem);
    }

    [Fact]
    public void FormatClockParts_GivesNoDesignatorOnA24HourCulture()
    {
        // Empty rather than absent, so the page can collapse its own element without asking what
        // kind of clock the machine has.
        var (digits, meridiem) = AmbientFormatter.FormatClockParts(
            new DateTime(2026, 9, 7, 14, 5, 0), Tr);

        Assert.Equal("14:05", digits);
        Assert.Equal(string.Empty, meridiem);
    }

    [Fact]
    public void FormatClockParts_UsesAmBeforeNoonAndPmAfter()
    {
        Assert.Equal("AM", AmbientFormatter.FormatClockParts(new DateTime(2026, 9, 7, 9, 5, 0), EnUs).Designator);
        Assert.Equal("PM", AmbientFormatter.FormatClockParts(new DateTime(2026, 9, 7, 21, 5, 0), EnUs).Designator);

        // Noon and midnight are the two the hour comparison can get backwards.
        Assert.Equal("PM", AmbientFormatter.FormatClockParts(new DateTime(2026, 9, 7, 12, 0, 0), EnUs).Designator);
        Assert.Equal("AM", AmbientFormatter.FormatClockParts(new DateTime(2026, 9, 7, 0, 0, 0), EnUs).Designator);
    }

    [Fact]
    public void FormatClockParts_LeavesNoTrailingSpaceWhereTheDesignatorWas()
    {
        // "h:mm tt" with the token removed leaves "h:mm ", which draws a gap before nothing and
        // pushes the digits off centre.
        var (digits, _) = AmbientFormatter.FormatClockParts(new DateTime(2026, 9, 7, 14, 5, 0), EnUs);

        Assert.Equal(digits.Trim(), digits);
    }

    [Fact]
    public void FormatClockParts_DropsSecondsLikeFormatClockDoes()
    {
        var seconds = (CultureInfo)EnUs.Clone();
        seconds.DateTimeFormat.ShortTimePattern = "h:mm:ss tt";

        var (digits, meridiem) = AmbientFormatter.FormatClockParts(
            new DateTime(2026, 9, 7, 14, 5, 9), seconds);

        Assert.Equal("2:05", digits);
        Assert.Equal("PM", meridiem);
    }

    [Fact]
    public void FormatClockParts_AgreesWithFormatClockAboutTheTime()
    {
        // The two must never describe one moment differently: the page draws the parts and the
        // screen reader is given FormatClock's whole string.
        var now = new DateTime(2026, 9, 7, 14, 5, 0);
        var (whole, _) = AmbientFormatter.FormatClock(now, EnUs);
        var (digits, meridiem) = AmbientFormatter.FormatClockParts(now, EnUs);

        Assert.Contains(digits, whole);
        Assert.Contains(meridiem, whole);
    }
}
