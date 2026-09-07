using System.Runtime.InteropServices;

namespace Plith.Services;

/// <summary>The three fields of SYSTEM_POWER_STATUS the ambient row cares about, lifted out
/// of the interop struct so the decision half can be tested without a P/Invoke.</summary>
public readonly record struct BatteryStatusRaw(byte BatteryFlag, byte BatteryLifePercent, byte ACLineStatus);

/// <summary>
/// Reads the system power status. Gather half only — every interpretation of these bytes
/// lives in AmbientFormatter.FormatBattery.
///
/// GetSystemPowerStatus rather than WinRT's PowerManager or WinForms' SystemInformation:
/// the project deliberately uses WpfScreenHelper to stay off System.Windows.Forms, and adding
/// that reference for one struct would undo it.
/// </summary>
public static class BatteryReader
{
    public static BatteryStatusRaw? Read()
    {
        // Returns null rather than throwing or inventing a value. The caller's contract is
        // that a failed read collapses the column, which is the same outcome as a desktop —
        // and on a desktop this is not an error condition worth logging every second.
        if (!GetSystemPowerStatus(out var s)) return null;
        return new BatteryStatusRaw(s.BatteryFlag, s.BatteryLifePercent, s.ACLineStatus);
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct SYSTEM_POWER_STATUS
    {
        public byte ACLineStatus;
        public byte BatteryFlag;
        public byte BatteryLifePercent;
        public byte SystemStatusFlag;
        public uint BatteryLifeTime;
        public uint BatteryFullLifeTime;
    }

    [DllImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool GetSystemPowerStatus(out SYSTEM_POWER_STATUS lpSystemPowerStatus);
}
