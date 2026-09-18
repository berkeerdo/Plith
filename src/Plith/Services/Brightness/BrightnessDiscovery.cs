using System.Runtime.InteropServices;

namespace Plith.Services.Brightness;

/// <summary>
/// Finds the displays whose brightness this machine can actually change.
/// </summary>
public static class BrightnessDiscovery
{
    /// <summary>
    /// The candidates that answered a read, in order.
    ///
    /// A read is the entire capability test. The alternative, GetMonitorCapabilities, was
    /// measured returning false with caps=0x0 on a monitor whose brightness reads and writes
    /// both work, so it can only produce false negatives here.
    /// </summary>
    public static IReadOnlyList<IBrightnessDevice> KeepAnswering(IEnumerable<IBrightnessDevice> candidates)
    {
        var kept = new List<IBrightnessDevice>();
        foreach (var candidate in candidates)
            if (candidate.TryRead(out _)) kept.Add(candidate);
        return kept;
    }

    /// <summary>
    /// Every physical monitor attached right now that answers a brightness read.
    ///
    /// Not unit-tested, and cannot be: it enumerates real display handles. The decision it
    /// makes is in <see cref="KeepAnswering"/>, which is.
    /// </summary>
    public static IReadOnlyList<IBrightnessDevice> Discover()
    {
        var candidates = new List<IBrightnessDevice>();

        try
        {
            var monitors = new List<nint>();
            var callback = new MonitorEnumProc((handle, _, _, _) => { monitors.Add(handle); return true; });
            _ = EnumDisplayMonitors(0, 0, callback, 0);
            GC.KeepAlive(callback);

            foreach (var monitor in monitors)
            {
                if (!GetNumberOfPhysicalMonitorsFromHMONITOR(monitor, out var count) || count == 0) continue;

                var physical = new PHYSICAL_MONITOR[count];
                if (!GetPhysicalMonitorsFromHMONITOR(monitor, count, physical)) continue;

                for (var i = 0; i < physical.Length; i++)
                {
                    var description = physical[i].szPhysicalMonitorDescription;
                    candidates.Add(new DdcBrightnessDevice(
                        physical[i].hPhysicalMonitor,
                        // The description is not unique on its own: two identical monitors
                        // both report the same string. The index disambiguates them.
                        $"{description}#{i}"));
                }
            }
        }
        catch (DllNotFoundException)
        {
            // dxva2 is absent on stripped SKUs. No devices is a valid answer.
            return [];
        }

        var kept = KeepAnswering(candidates);

        // Anything that did not answer holds an open handle nobody will use.
        foreach (var candidate in candidates)
            if (!kept.Contains(candidate) && candidate is IDisposable disposable) disposable.Dispose();

        return kept;
    }

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    private struct PHYSICAL_MONITOR
    {
        public nint hPhysicalMonitor;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 128)]
        public string szPhysicalMonitorDescription;
    }

    private delegate bool MonitorEnumProc(nint monitor, nint hdc, nint rect, nint data);

    [DllImport("user32.dll")]
    private static extern bool EnumDisplayMonitors(nint hdc, nint clip, MonitorEnumProc proc, nint data);

    [DllImport("dxva2.dll", SetLastError = true)]
    private static extern bool GetNumberOfPhysicalMonitorsFromHMONITOR(nint monitor, out uint count);

    [DllImport("dxva2.dll", SetLastError = true)]
    private static extern bool GetPhysicalMonitorsFromHMONITOR(nint monitor, uint count, [Out] PHYSICAL_MONITOR[] array);
}
