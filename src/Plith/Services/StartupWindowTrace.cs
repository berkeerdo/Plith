using System.Globalization;
using System.Text;

namespace Plith.Services;

/// <summary>
/// The phases inside <see cref="StartupPhase.Window"/>, in the order App.OnStartup runs them.
/// The order is the contract, exactly as it is for <see cref="StartupPhase"/>: the span of each
/// one is the distance from the previous mark, so renaming a member is free and reordering one
/// silently re-attributes time.
/// </summary>
public enum StartupWindowPhase
{
    /// <summary>OsdHost's field initializers and the BandWindow base constructor: everything that
    /// runs before the first statement of the constructor body, which is the earliest point a
    /// mark of ours can reach. The widget frame is built here, as a field initializer.</summary>
    Fields,
    /// <summary>The shell view model, the Classic presentation, the open trace and the hover
    /// poller with its subscriptions.</summary>
    Shell,
    /// <summary>The z-band query, the UIAccess log line — which is the first write to OsdHost's
    /// own DiagnosticLog and therefore opens the file — and the window property setters.</summary>
    Band,
    /// <summary>The OsdContent constructor — XAML parsed, the shared dictionaries first touched —
    /// plus ApplyShellWidth and handing it to Content.</summary>
    Content,
    /// <summary>BuildWidgetPages and SetPages. ONE page at this point, the clock; the weather,
    /// media and shelf pages arrive later from AttachAudioSource and AttachShelf, which run after
    /// the window phase is over.</summary>
    Pages,
    /// <summary>The accent mirror and the rest of the constructor's event wiring.</summary>
    Accent,
    /// <summary>CreateWindow: the native banded HWND and the HwndSource over it.</summary>
    Hwnd,
    /// <summary>ApplyPresentationMode: builds the presentation, reshapes the content, measures it
    /// through Reposition, and parks the strip.</summary>
    Present,
    /// <summary>CardHost.Start, plus the card wiring beside it in App.</summary>
    Host,
    /// <summary>WeatherService.Start.</summary>
    Weather,
}

/// <summary>
/// Times the inside of the launch's largest phase.
///
/// This exists because section 6 of docs/PERF-VERIFICATION.md found 337 to 368 ms in the `window`
/// column — the single largest thing Plith does at startup, by a factor of two and a half over
/// the next one — and said so with a phase name rather than a cause. Section 8 listed "what is
/// inside the window phase" as the next thing to measure and the first place worth trying to
/// move. Nothing can be moved until it is known which half of a constructor it is in.
///
/// Ten consecutive spans that PARTITION the phase rather than overlapping it, the same property
/// <see cref="StartupTrace"/> and <see cref="NotchOpenTrace"/> hold. Adding them up lands on the
/// total, so no column can be blamed twice. What each one covers is documented on
/// <see cref="StartupWindowPhase"/> rather than repeated here.
///
/// It was eight spans on its first run, and the three columns that carried the phase were named
/// `shell`, `hwnd` and `weather`. Two of those were causes and one was still a grab bag: `shell`
/// read 139 to 142 ms across a dozen statements, which is a phase name of the same kind the split
/// existed to get rid of. Split again into `fields`, `shell` and `band`, and the answer moved.
///
/// HOW IT LINES UP WITH THE COLUMN IT ZOOMS INTO, and this is the one thing a reader must not
/// take on trust. The zero is a clock reading of its own, taken one statement after
/// StartupTrace marks <see cref="StartupPhase.Cards"/>; the last mark is taken one statement
/// before StartupTrace marks <see cref="StartupPhase.Window"/>. Two pairs of readings of the same
/// monotonic clock, each pair separated by an object allocation or a method return, so this
/// total can land a millisecond either side of the `window` column and never further. That
/// closeness is the positive control on the whole instrument: a sub-total that does NOT land on
/// the window column means a mark is in the wrong place, not that the launch varied.
///
/// No stall figure here, deliberately. <see cref="UiStallWatch"/> is already armed across the
/// whole launch, this phase included, and its probe cannot tick while this phase holds the
/// thread, so a second stall measurement over the same window would be the same block reported
/// twice under two names.
///
/// Wall-clock only, and the caller supplies the clock, so the tests drive it and the app hands it
/// the same monotonic one Main started.
/// </summary>
public sealed class StartupWindowTrace
{
    private static readonly string[] PhaseNames =
        ["fields", "shell", "band", "content", "pages", "accent", "hwnd", "present", "host", "weather"];

    private readonly Func<long> _clockMs;
    private readonly long _zero;

    private readonly long[] _marks = new long[PhaseNames.Length];
    private readonly bool[] _marked = new bool[PhaseNames.Length];

    private bool _reported;

    /// <param name="clockMs">Monotonic, and the same clock <see cref="StartupTrace"/> is reading.
    /// The zero is taken here, which is why the caller must construct this immediately before the
    /// phase begins rather than anywhere convenient.</param>
    public StartupWindowTrace(Func<long> clockMs)
    {
        _clockMs = clockMs;
        _zero = clockMs();
    }

    /// <summary>
    /// The named phase just finished.
    ///
    /// Only the first mark of a phase counts, for the reason <see cref="StartupTrace.Mark"/>
    /// gives: a phase stamped again later would move its boundary forward and take the following
    /// phase's time with it. It matters more here than there, because OsdHost's constructor is
    /// reachable more than once in a test host while App.OnStartup is not.
    /// </summary>
    public void Mark(StartupWindowPhase phase)
    {
        var i = (int)phase;
        if ((uint)i >= (uint)_marked.Length || _marked[i]) return;

        _marked[i] = true;
        _marks[i] = _clockMs();
    }

    /// <summary>
    /// Format the line, once. Returns null on every call after the first.
    ///
    /// The caller reports this from the stall probe's tick rather than at the end of the phase,
    /// alongside the launch line it belongs to. Not for the reason StartupTrace has to — there is
    /// no stall figure here to wait for — but because writing it inside OnStartup would put a log
    /// write into the `audio` span, which is to say this instrument would appear in a column of
    /// the measurement it exists to explain.
    /// </summary>
    public string? Report()
    {
        if (_reported) return null;
        _reported = true;

        // An unmarked phase reads zero and the next one absorbs its span, so the columns still
        // add up to the total on a launch where a phase returned early. Clamping to the previous
        // mark does the same for a mark that arrived out of order: a negative column would be
        // read as a fast phase rather than as a broken instrument.
        var line = new StringBuilder(160);
        var spans = new StringBuilder(140);

        long previous = _zero;
        for (var i = 0; i < PhaseNames.Length; i++)
        {
            var end = _marked[i] ? Math.Max(previous, _marks[i]) : previous;
            if (i > 0) spans.Append(", ");
            spans.Append(CultureInfo.InvariantCulture, $"{PhaseNames[i]} {end - previous}");
            previous = end;
        }

        line.Append(CultureInfo.InvariantCulture, $"Window: {previous - _zero}ms total ({spans})");
        return line.ToString();
    }
}
