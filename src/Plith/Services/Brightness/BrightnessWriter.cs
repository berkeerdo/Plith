namespace Plith.Services.Brightness;

/// <summary>
/// Writes brightness to every device, newest value wins, one write in flight.
///
/// A DDC/CI write was measured at 56 ms on the monitor this was built against, so a held key
/// produces far more requests than the hardware can take. A queue would keep changing the
/// screen after the key came up; this replaces the pending value instead, so the screen ends
/// where the person left it and gets there as fast as the bus allows.
///
/// Every implementation examined converges on this shape: Monitorian queues writes on a
/// background worker, and the laptop-key bridge that measured 65-71 ms per write collapses
/// rapid presses to the newest value.
/// </summary>
public sealed class BrightnessWriter
{
    private readonly IReadOnlyList<IBrightnessDevice> _devices;
    private readonly Action<Action> _runner;
    private readonly object _gate = new();

    private int? _pending;
    private bool _pumping;

    /// <param name="runner">How the pump gets off the calling thread. Defaults to the thread
    /// pool. A test passes something it controls, because a pump on the thread pool makes
    /// every assertion about coalescing a race.</param>
    public BrightnessWriter(IReadOnlyList<IBrightnessDevice> devices, Action<Action>? runner = null)
    {
        _devices = devices;
        _runner = runner ?? (work => Task.Run(work));
    }

    /// <summary>Raised on the pump's thread after a value has been written to every device.
    /// Subscribers that touch UI must marshal; see CardHost's note on the dispatcher.</summary>
    public event Action<int>? Wrote;

    /// <summary>Raised on the pump's thread with the id of a device that refused a write. The
    /// write itself is not retried and the other devices still get the value; this exists so a
    /// display that answers nothing leaves a trace instead of looking like one that works.</summary>
    public event Action<string>? Refused;

    public void Request(int value)
    {
        lock (_gate)
        {
            _pending = value;
            if (_pumping) return;
            _pumping = true;
        }

        _runner(Pump);
    }

    private void Pump()
    {
        while (true)
        {
            int value;
            lock (_gate)
            {
                if (_pending is null)
                {
                    _pumping = false;
                    return;
                }

                value = _pending.Value;
                _pending = null;
            }

            foreach (var device in _devices)
            {
                // A refusal is announced but never retried and never fatal. One monitor
                // refusing is not a reason to leave the others where they were, and a display
                // that has gone away must not take the gesture down with it.
                if (!device.TryWrite(value)) Refused?.Invoke(device.Id);
            }

            Wrote?.Invoke(value);
        }
    }
}
