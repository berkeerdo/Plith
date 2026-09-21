using System.Runtime.InteropServices;

namespace Plith.Services.Brightness;

/// <summary>
/// One external display, reached over DDC/CI through dxva2.dll.
///
/// The handle is opened once and kept. A write was measured at 56 ms on the monitor this was
/// built against, and that is the DDC/CI exchange itself; reopening the handle per write
/// would add to a cost that already limits how fast brightness can move.
/// </summary>
public sealed class DdcBrightnessDevice : IBrightnessDevice, IDisposable
{
    private nint _handle;
    private bool _disposed;

    internal DdcBrightnessDevice(nint physicalMonitorHandle, string id)
    {
        _handle = physicalMonitorHandle;
        Id = id;
    }

    public string Id { get; }

    public bool TryRead(out BrightnessReading reading)
    {
        reading = default;
        if (_disposed || _handle == 0) return false;

        // Deliberately NOT preceded by GetMonitorCapabilities. Measured on a PG27AQDM: that
        // call returns false with caps=0x0 while this one answers 0/30/100. Gating on it
        // would report the feature unsupported on hardware where it works.
        if (!GetMonitorBrightness(_handle, out var min, out var current, out var max)) return false;

        reading = new BrightnessReading((int)min, (int)current, (int)max);
        return true;
    }

    public bool TryWrite(int value)
    {
        if (_disposed || _handle == 0) return false;
        return SetMonitorBrightness(_handle, (uint)value);
    }

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        if (_handle != 0)
        {
            _ = DestroyPhysicalMonitor(_handle);
            _handle = 0;
        }
    }

    [DllImport("dxva2.dll", SetLastError = true)]
    private static extern bool GetMonitorBrightness(nint handle, out uint min, out uint current, out uint max);

    [DllImport("dxva2.dll", SetLastError = true)]
    private static extern bool SetMonitorBrightness(nint handle, uint value);

    [DllImport("dxva2.dll", SetLastError = true)]
    private static extern bool DestroyPhysicalMonitor(nint handle);
}
