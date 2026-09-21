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
}
