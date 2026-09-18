namespace Plith.Services.Brightness;

/// <summary>
/// One display whose brightness can be read and written.
///
/// Both methods return false rather than throwing. A monitor can be unplugged between two
/// calls, and a display that has gone away must not take the OSD down with it.
/// </summary>
public interface IBrightnessDevice
{
    /// <summary>Stable for the life of the device. Used in logs and in settings.</summary>
    string Id { get; }

    bool TryRead(out BrightnessReading reading);

    bool TryWrite(int value);
}
