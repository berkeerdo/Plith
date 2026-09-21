using System.Globalization;
using System.Management;

namespace Plith.Services.Brightness;

/// <summary>
/// A laptop's built-in panel, reached through WMI.
///
/// The other half of the act side, and the one this project cannot test. A built-in display is
/// not normally reachable over DDC/CI, so the dxva2 path finds nothing on a laptop; WMI is the
/// path Windows itself uses for the brightness slider. The machine this was written on is a
/// desktop, which means every line here is reasoned from the documentation rather than measured,
/// and that is recorded rather than glossed over.
///
/// The read and the write live in different WMI classes: WmiMonitorBrightness carries the
/// current value, WmiMonitorBrightnessMethods carries WmiSetBrightness. They are matched by
/// InstanceName.
/// </summary>
public sealed class WmiBrightnessDevice : IBrightnessDevice
{
    /// <summary>Seconds WmiSetBrightness may take before the call gives up. One is generous for
    /// a local panel and short enough that a wedged provider cannot hold a key press.</summary>
    private const uint WriteTimeoutSeconds = 1;

    private readonly string _instanceName;

    internal WmiBrightnessDevice(string instanceName)
    {
        _instanceName = instanceName;
        Id = "Internal panel " + instanceName;
    }

    public string Id { get; }

    /// <summary>
    /// Every built-in panel this machine reports, or an empty list.
    ///
    /// Returns nothing on a desktop, where the class has no instances, and that is the normal
    /// case rather than a failure.
    /// </summary>
    public static IReadOnlyList<IBrightnessDevice> Enumerate()
    {
        var found = new List<IBrightnessDevice>();

        try
        {
            using var searcher = new ManagementObjectSearcher(
                new ManagementScope(@"\\.\root\wmi"),
                new ObjectQuery("SELECT InstanceName FROM WmiMonitorBrightness"));
            using var results = searcher.Get();

            foreach (var instance in results)
            {
                using (instance)
                {
                    if (instance["InstanceName"] is string name && !string.IsNullOrEmpty(name))
                        found.Add(new WmiBrightnessDevice(name));
                }
            }
        }
        catch (ManagementException)
        {
            // "Not supported" is what a desktop answers. Nothing to add.
        }
        catch (UnauthorizedAccessException)
        {
        }

        return found;
    }

    public bool TryRead(out BrightnessReading reading)
    {
        reading = default;

        try
        {
            using var searcher = new ManagementObjectSearcher(
                new ManagementScope(@"\\.\root\wmi"),
                new ObjectQuery("SELECT CurrentBrightness FROM WmiMonitorBrightness WHERE InstanceName = '" + EscapeForWql(_instanceName) + "'"));
            using var results = searcher.Get();

            foreach (var instance in results)
            {
                using (instance)
                {
                    var current = Convert.ToInt32(instance["CurrentBrightness"], CultureInfo.InvariantCulture);

                    // 0 to 100 rather than the Level array this class also exposes. WMI defines
                    // CurrentBrightness as a percentage, and a panel that supports only some of
                    // those values snaps to its own nearest one. Reading the array would move
                    // that decision here for no gain.
                    reading = new BrightnessReading(0, current, 100);
                    return true;
                }
            }
        }
        catch (ManagementException) { }
        catch (UnauthorizedAccessException) { }

        return false;
    }

    public bool TryWrite(int value)
    {
        try
        {
            using var searcher = new ManagementObjectSearcher(
                new ManagementScope(@"\\.\root\wmi"),
                new ObjectQuery("SELECT * FROM WmiMonitorBrightnessMethods WHERE InstanceName = '" + EscapeForWql(_instanceName) + "'"));
            using var results = searcher.Get();

            foreach (var instance in results)
            {
                using (instance as ManagementObject)
                {
                    if (instance is not ManagementObject method) continue;

                    method.InvokeMethod("WmiSetBrightness",
                        new object[] { WriteTimeoutSeconds, (byte)Math.Clamp(value, 0, 100) });
                    return true;
                }
            }
        }
        catch (ManagementException) { }
        catch (UnauthorizedAccessException) { }

        return false;
    }

    /// <summary>
    /// A WMI instance name goes into a WQL string literal, and it is full of backslashes:
    /// "DISPLAY\\AUS27FD\\5&amp;efad5ff&amp;0&amp;UID4353_0". A backslash escapes the next character in
    /// WQL, so an unescaped name silently matches nothing and the panel looks absent.
    /// </summary>
    private static string EscapeForWql(string value)
        => value.Replace("\\", "\\\\", StringComparison.Ordinal)
                .Replace("'", "\\'", StringComparison.Ordinal);
}
