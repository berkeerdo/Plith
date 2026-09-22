using System.Globalization;
using System.Text;

namespace Plith.Services;

/// <summary>
/// The phases of a launch, in the order App.OnStartup runs them. The order is the contract:
/// <see cref="StartupTrace"/> computes each span as the distance from the previous phase's mark,
/// so renaming a member is free and reordering one silently re-attributes time.
/// </summary>
public enum StartupPhase
{
    /// <summary>Main to OnStartup: the App constructor, InitializeComponent, and Run reaching us.</summary>
    App,
    /// <summary>The log, SettingsService.Load, the Run-key reconcile, and ThemeService.</summary>
    Settings,
    /// <summary>The SMTC client, the four cards, weather, the fullscreen watcher, CardHost.</summary>
    Cards,
    /// <summary>The OsdHost constructor, which creates the native HWND and builds the content and
    /// the first widget page, plus CardHost.Start and WeatherService.Start. Split ten ways by
    /// <see cref="StartupWindowTrace"/>, because this is the launch's largest span.</summary>
    Window,
    /// <summary>The microphone endpoint and the orchestrator: Core Audio enumeration, and the
    /// Voicemeeter probe on a machine that has it.</summary>
    Audio,
    /// <summary>The foreground watcher, the flyout suppressor, the low-level volume key hook,
    /// and the summon hotkey.</summary>
    Hooks,
    /// <summary>Brightness and the tray icon.</summary>
    Tray,
    /// <summary>The shelf store, the pipe, and launching the drop catcher process.</summary>
    Shelf,
}

/// <summary>
/// Times one launch of Plith, from the moment the process was created to the moment the UI thread
/// first goes idle.
///
/// This exists because section 6 of docs/PERF-VERIFICATION.md named app startup the prime suspect
/// for a reported symptom and had nothing to measure it with. The notch's own first open was
/// cleared at 26 to 36 ms by <see cref="NotchOpenTrace"/>, which is about two per cent of the
/// reported second, so the second is somewhere else and the report said "first use" rather than
/// "first open". Nothing in this repo had ever timed what happens between launching Plith and the
/// notch being ready.
///
/// Ten consecutive spans that PARTITION the launch rather than overlapping it. Adding them up
/// lands on the total, so no column can be blamed twice. Nine of them are named by
/// <see cref="StartupPhase"/> plus the runtime's own start; the tenth is the settle:
///
///   clr     process creation to the first statement of Main. The CLR starting, WPF's assemblies
///           loading, and the startup path being jitted. Never timed before, and the one span
///           that is over before any code of ours can stamp a clock.
///   settle  OnStartup returning to the UI thread going idle. Posted at ContextIdle, which WPF
///           runs only once everything above it has drained, so this is where work queued by the
///           launch but not done inside it appears. The first paint of the notch is in here.
///
/// The stall is the separate half and the only figure that measures the reported symptom, because
/// a launch that takes a second while pumping input does not put the busy cursor on screen and a
/// launch that blocks for 300 ms does. It is measured by <see cref="UiStallWatch"/> and passed in
/// at <see cref="Settled"/> rather than computed here: this type owns the spans, that one owns the
/// blocking, and the watch outlives the launch while this does not.
///
/// Wall-clock only, and deliberately: the caller supplies the clock, so the tests drive it and the
/// app hands it a monotonic one.
///
/// HONEST WEAKNESS, and it applies to exactly one column. Every span but `clr` is a difference of
/// two readings from one monotonic Stopwatch. `clr` cannot be, because the interval it measures
/// ended before the Stopwatch could exist: it is the distance between the process creation time
/// the OS recorded and a DateTime sampled in Main. Two clocks, one of them wall-clock, and its
/// resolution is the system timer rather than the performance counter. It is good to a few
/// milliseconds, which is fine for a column expected to run into the hundreds, and it is NOT the
/// same quality of measurement as the nine beside it.
/// </summary>
public sealed class StartupTrace
{
    private static readonly string[] PhaseNames =
        ["app", "settings", "cards", "window", "audio", "hooks", "tray", "shelf"];

    private readonly Func<long> _clockMs;
    private readonly long _clrMs;

    private readonly long[] _marks = new long[PhaseNames.Length];
    private readonly bool[] _marked = new bool[PhaseNames.Length];

    private bool _idle;
    private long _idleAt;
    private bool _reported;

    /// <param name="clockMs">Monotonic, and zero at the first statement of Main.</param>
    /// <param name="clrMs">What the runtime's own start cost, measured before this type existed.</param>
    public StartupTrace(Func<long> clockMs, long clrMs)
    {
        _clockMs = clockMs;
        _clrMs = clrMs;
    }

    /// <summary>
    /// The named phase just finished.
    ///
    /// Only the first mark of a phase counts. Nothing calls this twice today, but a phase stamped
    /// again later would move its boundary forward and take the following phase's time with it.
    /// </summary>
    public void Mark(StartupPhase phase)
    {
        if (_idle) return;

        var i = (int)phase;
        if ((uint)i >= (uint)_marked.Length || _marked[i]) return;

        _marked[i] = true;
        _marks[i] = _clockMs();
    }

    /// <summary>
    /// The UI thread went idle after the launch. Stamps the end of the settle span and nothing
    /// else.
    ///
    /// One-shot, because a launch is not a thing that happens twice. The caller posts this at
    /// ContextIdle and WPF can run that more than once; a second stamp would move the end of the
    /// launch to some later moment that has nothing to do with it.
    /// </summary>
    public void Idle()
    {
        if (_idle) return;
        _idle = true;
        _idleAt = _clockMs();
    }

    /// <summary>
    /// Format the line, once the stall figures exist to put in it. Returns null before
    /// <see cref="Idle"/> has been reached, and on every call after the first.
    ///
    /// Separate from <see cref="Idle"/> because of a defect the first version of this instrument
    /// had on hardware, found by running it rather than by reading it. Formatting at the settle
    /// meant the stall column was filled in before the probe had ever ticked: Input priority sits
    /// above ContextIdle, but the probe's FIRST tick is one interval away, and the settle
    /// routinely arrives sooner. So the column read "0ms over 0 ticks" on every launch. Honest,
    /// because the count said nothing had been sampled, and useless, because it always would.
    ///
    /// The caller therefore reports from the probe's tick, by which time there is a real gap to
    /// report, while the settle span is still measured at the moment it actually happened.
    /// </summary>
    public string? Report(long maxStallMs, int stallTicks)
    {
        if (!_idle || _reported) return null;
        _reported = true;

        var now = _idleAt;

        var line = new StringBuilder(220);
        line.Append(CultureInfo.InvariantCulture, $"Startup: {_clrMs + now}ms total (clr {_clrMs}");

        // An unmarked phase reads zero and the next one absorbs its span, so the columns still
        // add up to the total on a launch where a phase returned early. Clamping to the previous
        // mark does the same for a mark that arrived out of order: a negative column would be
        // read as a fast phase rather than as a broken instrument.
        long previous = 0;
        for (var i = 0; i < PhaseNames.Length; i++)
        {
            var end = _marked[i] ? Math.Max(previous, _marks[i]) : previous;
            line.Append(CultureInfo.InvariantCulture, $", {PhaseNames[i]} {end - previous}");
            previous = end;
        }

        line.Append(CultureInfo.InvariantCulture,
            $", settle {Math.Max(0, now - previous)}), max UI stall {maxStallMs}ms over {stallTicks} ticks");

        return line.ToString();
    }
}
