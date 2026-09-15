using System.Runtime.InteropServices;

namespace Plith.Services;

/// <summary>
/// Display brightness, over DDC/CI.
///
/// The obvious path was WMI's <c>WmiMonitorBrightness</c>, and it was written first and thrown
/// away: it reaches the built-in panel of a laptop and nothing else, so on a desktop — which is
/// what this is developed on — it is code that can never once run. DDC/CI talks to the monitor
/// over the video cable instead, which is the case a desktop actually has. Measured here before
/// any of this was written: the attached panel answers with a 0..100 range.
///
/// Laptops are the mirror of that gap. Many internal panels answer DDC/CI too, and the ones that
/// do not will need the WMI path back — as a fallback behind this, added when there is a machine
/// to prove it on rather than now, on faith.
///
/// Every call here is slow in a way an in-process API never is: it is I2C traffic down the
/// display cable, tens of milliseconds at best, occasionally far worse. That single fact shapes
/// the whole class — see <see cref="TrySet"/>.
/// </summary>
public sealed class BrightnessClient : IDisposable
{
    private readonly DiagnosticLog? _log;
    private readonly object _gate = new();

    private IntPtr _monitor;
    private uint _min;
    private uint _max;
    private int _current = -1;

    /// <summary>The value the display should end up at, or -1 for "nothing outstanding".</summary>
    private int _pending = -1;
    private bool _writerRunning;
    private bool _disposed;

    public BrightnessClient(DiagnosticLog? log = null) => _log = log;

    /// <summary>Brightness as 0..100, or null when no display answered. Null means "this machine
    /// has no brightness Plith can reach", which is a real state — a desktop on a monitor with
    /// DDC/CI switched off in its own menu lands here — and must show no control rather than a
    /// dead one.</summary>
    public int? Current => _current < 0 ? null : _current;

    /// <summary>Raised after a write lands, carrying the value the display is now at. Raised on
    /// the writer thread, never the UI one: subscribers hop their own dispatcher, the same
    /// contract <see cref="MicrophoneClient"/>.Changed has.</summary>
    public event Action<int>? Changed;

    /// <summary>
    /// Attach to the primary display and read it once.
    ///
    /// The primary specifically, not the monitor the notch is pinned to. Brightness is a property
    /// of a screen rather than of the overlay, and a person who moves the notch to a second
    /// monitor has not asked to start dimming that one instead — a control that silently changes
    /// which display it governs is worse than one that is clear about governing a single one.
    /// </summary>
    public bool Start()
    {
        lock (_gate)
        {
            if (_disposed) return false;
            if (_monitor != IntPtr.Zero) return true;

            var hMonitor = MonitorFromPoint(default, MonitorDefaultToPrimary);
            if (hMonitor == IntPtr.Zero) return Unavailable("no primary monitor handle");

            uint count = 0;
            if (!GetNumberOfPhysicalMonitorsFromHMONITOR(hMonitor, ref count) || count == 0)
                return Unavailable("no physical monitors behind the handle");

            var physical = new PhysicalMonitor[count];
            if (!GetPhysicalMonitorsFromHMONITOR(hMonitor, count, physical))
                return Unavailable("could not open the physical monitors");

            // The first one, and the rest are released immediately. A single HMONITOR maps to
            // more than one physical panel only in mirrored and daisy-chained setups, where
            // there is no single right answer anyway; holding handles we will never write to
            // would just be a leak with a story attached.
            _monitor = physical[0].Handle;
            if (count > 1) ReleaseAllBut(physical, 0);

            uint min = 0, current = 0, max = 0;
            if (!GetMonitorBrightness(_monitor, ref min, ref current, ref max))
            {
                DestroyPhysicalMonitor(_monitor);
                _monitor = IntPtr.Zero;
                return Unavailable("the display did not answer GetMonitorBrightness");
            }

            _min = min;
            _max = max;
            _current = ToPercent(current);

            _log?.Info("Brightness", $"'{physical[0].Description}' answered: {_current}% (raw {current} in {min}..{max}).");
            return true;
        }
    }

    private bool Unavailable(string why)
    {
        // Info, not a warning. A monitor with DDC/CI disabled, a VM, an RDP session and a KVM in
        // the path all land here, and none of them is a fault - there is simply nothing to offer.
        _log?.Info("Brightness", $"Unavailable: {why}.");
        _current = -1;
        return false;
    }

    /// <summary>
    /// Ask the display to move to a percentage.
    ///
    /// Returns as soon as the request is recorded, because a DDC/CI write is far too slow to sit
    /// in front of a mouse move. Writes happen on one background thread holding a single pending
    /// value, so a drag that passes through forty intermediate positions performs however many
    /// writes the cable manages and lands on the last one — never a queue of forty draining
    /// visibly after the finger has stopped.
    ///
    /// The reported value moves immediately rather than waiting for the cable, so the control
    /// tracks the finger. That is a deliberate lie of exactly one kind: it claims the display
    /// WILL be there, and <see cref="Changed"/> corrects it if the display disagrees.
    /// </summary>
    public bool TrySet(int percent)
    {
        var clamped = Math.Clamp(percent, 0, 100);

        lock (_gate)
        {
            if (_disposed || _monitor == IntPtr.Zero) return false;

            _pending = clamped;
            _current = clamped;

            if (_writerRunning) return true;
            _writerRunning = true;
        }

        // Long-running by the standard of the pool's expectations - a write can block for the
        // better part of a second on an unhappy cable - so it gets its own thread rather than
        // occupying a pool one.
        var writer = new Thread(WriteLoop)
        {
            IsBackground = true,
            Name = "Plith.Brightness",
        };
        writer.Start();
        return true;
    }

    private void WriteLoop()
    {
        while (true)
        {
            int target;
            IntPtr monitor;

            lock (_gate)
            {
                if (_disposed || _pending < 0 || _monitor == IntPtr.Zero)
                {
                    _writerRunning = false;
                    return;
                }

                target = _pending;
                _pending = -1;
                monitor = _monitor;
            }

            var raw = ToRaw(target);
            if (!SetMonitorBrightness(monitor, raw))
            {
                _log?.Warn("Brightness", $"Display refused {target}% (raw {raw}).");

                // Stop after a refusal rather than retrying. A display that says no to one value
                // says no to the next, and a retry loop on a cable this slow turns one failure
                // into a thread that never leaves.
                lock (_gate) { _pending = -1; _writerRunning = false; }

                // Take back the optimistic value. TrySet moved the reported reading the instant
                // the finger did, on the promise that the display would follow; it did not, so
                // the promise has to be withdrawn rather than left standing as the truth.
                var actual = Refresh();
                if (actual is { } corrected) Changed?.Invoke(corrected);
                return;
            }

            Changed?.Invoke(target);
        }
    }

    /// <summary>
    /// Re-read the display.
    ///
    /// Brightness changes from outside Plith — the monitor's own buttons, another utility — and
    /// DDC/CI offers no notification at all, so the value is fetched when it is about to be shown
    /// rather than pushed. Slow enough that this must not be called from a paint.
    /// </summary>
    public int? Refresh()
    {
        lock (_gate)
        {
            if (_disposed || _monitor == IntPtr.Zero) return null;

            // A read while a write is outstanding would return the value the display is moving
            // away from, and overwrite the value the finger is on with it.
            if (_pending >= 0 || _writerRunning) return Current;

            uint min = 0, current = 0, max = 0;
            if (!GetMonitorBrightness(_monitor, ref min, ref current, ref max)) return Current;

            _min = min;
            _max = max;
            _current = ToPercent(current);
            return _current;
        }
    }

    /// <summary>Raw display units to 0..100, against the range this display reported. The
    /// conversion itself lives in <see cref="BrightnessMath"/>, where the ranges no machine here
    /// has can be tested.</summary>
    private int ToPercent(uint raw) => BrightnessMath.ToPercent(raw, _min, _max);

    private uint ToRaw(int percent) => BrightnessMath.ToRaw(percent, _min, _max);

    private static void ReleaseAllBut(PhysicalMonitor[] monitors, int keep)
    {
        for (var i = 0; i < monitors.Length; i++)
        {
            if (i == keep) continue;
            DestroyPhysicalMonitor(monitors[i].Handle);
        }
    }

    public void Dispose()
    {
        IntPtr monitor;

        lock (_gate)
        {
            if (_disposed) return;
            _disposed = true;
            _pending = -1;
            monitor = _monitor;
            _monitor = IntPtr.Zero;
            _current = -1;
        }

        // Outside the lock: the writer may be inside a call that takes most of a second, and it
        // takes the same lock on the way out. Its next turn of the loop sees the handle gone.
        if (monitor != IntPtr.Zero) DestroyPhysicalMonitor(monitor);
    }

    private const uint MonitorDefaultToPrimary = 1;

    [StructLayout(LayoutKind.Sequential)]
    private struct NativePoint
    {
        public int X;
        public int Y;
    }

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    private struct PhysicalMonitor
    {
        public IntPtr Handle;

        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 128)]
        public string Description;
    }

    [DllImport("user32.dll")]
    private static extern IntPtr MonitorFromPoint(NativePoint pt, uint flags);

    [DllImport("dxva2.dll", SetLastError = true)]
    private static extern bool GetNumberOfPhysicalMonitorsFromHMONITOR(IntPtr monitor, ref uint count);

    [DllImport("dxva2.dll", SetLastError = true)]
    private static extern bool GetPhysicalMonitorsFromHMONITOR(IntPtr monitor, uint count, [Out] PhysicalMonitor[] physical);

    [DllImport("dxva2.dll", SetLastError = true)]
    private static extern bool GetMonitorBrightness(IntPtr physical, ref uint min, ref uint current, ref uint max);

    [DllImport("dxva2.dll", SetLastError = true)]
    private static extern bool SetMonitorBrightness(IntPtr physical, uint value);

    [DllImport("dxva2.dll", SetLastError = true)]
    private static extern bool DestroyPhysicalMonitor(IntPtr physical);
}
