using System.Globalization;

namespace Plith.Services.Brightness;

/// <summary>
/// Turns a held brightness key into two log lines instead of fifty.
///
/// A write was measured at 56 ms, so holding a key produces roughly fifteen steps a second.
/// Logging each one floods the file during exactly the gesture worth diagnosing, and a log
/// that has to be capped then throws away the context around it. So a run gets one line when
/// it starts and one summary when it ends, and the summary carries what a reader actually
/// needs: how many steps, where it started and finished, and whether any display refused.
///
/// A run ends when the key has been quiet for a moment or when the direction changes.
/// </summary>
public sealed class BrightnessRunLog
{
    /// <summary>How long after the last step a run is considered finished. Longer than the gap
    /// between repeats of a held key, short enough that the summary lands while the person is
    /// still looking at the screen.</summary>
    public static readonly TimeSpan DefaultGap = TimeSpan.FromMilliseconds(600);

    private readonly Action<string> _write;
    private readonly Action<TimeSpan, Action> _scheduleClose;
    private readonly TimeSpan _gap;

    private bool _open;
    private bool _up;
    private int _from;
    private int _last;
    private int _steps;
    private int _refusals;

    /// <param name="scheduleClose">Restarts the quiet timer. A test supplies its own so a run
    /// can be closed on demand rather than waited out.</param>
    public BrightnessRunLog(Action<string> write, Action<TimeSpan, Action> scheduleClose, TimeSpan? gap = null)
    {
        _write = write;
        _scheduleClose = scheduleClose;
        _gap = gap ?? DefaultGap;
    }

    /// <summary>One key press landed, moving the level from one value to another.</summary>
    public void Step(bool up, int from, int to)
    {
        // A direction change is a new gesture even with no pause between them, and a summary
        // spanning both would read as one long press that went nowhere.
        if (_open && up != _up) Close();

        if (!_open)
        {
            _open = true;
            _up = up;
            _from = from;
            _steps = 0;
            _refusals = 0;
            _write(string.Create(CultureInfo.InvariantCulture, $"{Direction(up)} {from} to {to}."));
        }

        _last = to;
        _steps++;
        _scheduleClose(_gap, Close);
    }

    /// <summary>A display refused a write during the current run.</summary>
    public void NoteRefusal(string deviceId)
    {
        if (!_open)
        {
            // Outside a run there is nothing to summarise it into, so it is worth a line of
            // its own: a refusal with no key press behind it is its own kind of strange.
            _write(string.Create(CultureInfo.InvariantCulture, $"Write refused by {deviceId} outside a run."));
            return;
        }

        _refusals++;
    }

    /// <summary>End the run and write its summary, if there is one worth writing.</summary>
    public void Close()
    {
        if (!_open) return;
        _open = false;

        // A single press already said everything in its opening line. Only a refusal adds
        // something the reader does not already have.
        if (_steps <= 1 && _refusals == 0) return;

        var refused = _refusals > 0
            ? string.Create(CultureInfo.InvariantCulture, $", {_refusals} refused")
            : "";

        _write(_steps <= 1
            ? string.Create(CultureInfo.InvariantCulture, $"held {Direction(_up)}: 1 step{refused}.")
            : string.Create(CultureInfo.InvariantCulture,
                $"held {Direction(_up)}: {_steps} steps, {_from} to {_last}{refused}."));
    }

    private static string Direction(bool up) => up ? "Brighter" : "Dimmer";
}
