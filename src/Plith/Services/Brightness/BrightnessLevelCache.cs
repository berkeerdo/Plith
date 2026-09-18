namespace Plith.Services.Brightness;

/// <summary>
/// The level this process last knew, so a key press does not have to ask the monitor.
///
/// Measured on the monitor this was built against: a DDC/CI read costs 60 ms and a write costs
/// 60 ms. Without this, one key press cost three round trips and about 180 ms: a read to work
/// out where the level was, the write itself, and another read to turn the written value into
/// the percentage the OSD shows. Two of those three were avoidable, and the delay was visible
/// as a sluggish key.
///
/// The cache is deliberately short-lived. Someone can change brightness from the monitor's own
/// buttons and nothing tells us, so a value kept forever would drift away from the screen. It
/// is invalidated when a run of presses ends, which means every gesture starts from a real
/// reading and only the presses inside it are free.
/// </summary>
public sealed class BrightnessLevelCache
{
    private BrightnessReading? _reading;

    public bool TryGet(out BrightnessReading reading)
    {
        if (_reading is { } cached)
        {
            reading = cached;
            return true;
        }

        reading = default;
        return false;
    }

    public void Set(BrightnessReading reading) => _reading = reading;

    /// <summary>
    /// A value was written, so that is where the level is now.
    ///
    /// Does nothing when there is no cached reading: the range came from a real read, and
    /// inventing one from a written value would guess at the minimum and the maximum.
    /// </summary>
    public void NoteWritten(int value)
    {
        if (_reading is not { } cached) return;
        _reading = cached with { Current = value };
    }

    public void Invalidate() => _reading = null;
}
