using System.Management;
using Plith.Services.Brightness;

namespace Plith.Tests;

/// <summary>
/// The monitor itself needs WMI and a dispatcher, so only its one piece of data is tested here:
/// the namespace path.
///
/// That is the piece that broke. It shipped as <c>\.\root\wmi</c>, one backslash short, and
/// every Plith that ever ran failed to subscribe. The failure was invisible because the catch
/// around it was written for a real case, a desktop with no internal panel, and reported the
/// bug in the same words at the same level. A laptop's brightness key therefore changed the
/// screen and Plith never showed anything. Diagnostics from the reporting machine, 2026-09-23:
/// "No brightness event source: ManagementException: Invalid parameter".
/// </summary>
public class BrightnessMonitorTests
{
    [Fact]
    public void ScopePath_Parses()
    {
        // ManagementScope validates in its constructor, so this is the whole test: the path
        // either is one or it is not. No WMI call is made and no display is touched.
        var ex = Record.Exception(() => new ManagementScope(BrightnessMonitor.ScopePath));

        Assert.Null(ex);
    }

    [Fact]
    public void ScopePath_NamesTheLocalMachine()
    {
        // The parse test above is the one that catches the defect. This one says why the path
        // has the shape it has, so a future edit that "tidies" the leading slashes fails with
        // a reason rather than with a WMI exception nobody can place.
        Assert.StartsWith(@"\\.\", BrightnessMonitor.ScopePath, StringComparison.Ordinal);
    }
}
