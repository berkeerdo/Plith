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
}
