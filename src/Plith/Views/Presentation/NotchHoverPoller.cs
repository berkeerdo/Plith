using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Threading;
using Plith.Services;

namespace Plith.Views.Presentation;

/// <summary>
/// Reports when the cursor enters or leaves the notch's resting rectangle.
///
/// This polls GetCursorPos rather than installing WH_MOUSE_LL on purpose. Mouse hooks run on
/// the input hot path, where a slow callback lags the whole system's cursor, and Plith
/// already carries one global hook (WH_KEYBOARD_LL) — a second one on a hotter path is the
/// larger risk. The resting notch is a few pixels tall and only needs to feel responsive to
/// a deliberate move toward it, which 60 ms comfortably covers.
///
/// The 60 ms is no longer flat. It applies while the cursor is near the notch or a button is
/// held, and the poller drops to 200 ms otherwise, which is where nearly all of its cost was:
/// docs/PERF-VERIFICATION.md section 10 measured the flat rate at about 340 context switches a
/// second, 80 to 85 per cent of everything the app does at rest. <see cref="NotchPollRate"/>
/// holds the rule and the reasoning.
///
/// The timer runs for as long as notch mode is the active presentation — including while the
/// panel is open, since the resting rectangle's own hover state still needs tracking to know
/// when the user has left it — and stops only when the mode switches away from the notch, so Classic
/// pays nothing for this.
/// </summary>
internal sealed class NotchHoverPoller : IDisposable
{
    private readonly DispatcherTimer _timer;
    private readonly DragApproachDetector _drag = new();
    private bool _wasInside;
    private bool _wasInsidePanel;

    private readonly DiagnosticLog? _log;

    public NotchHoverPoller(Dispatcher dispatcher, DiagnosticLog? log = null)
    {
        _log = log;
        _timer = new DispatcherTimer(DispatcherPriority.Background, dispatcher) { Interval = NotchPollRate.Fast };
        _timer.Tick += (_, _) => Poll();
    }

    /// <summary>The resting notch's screen rectangle in DIP. Set by OsdHost after every
    /// reposition. Its width is a constant, so it does NOT grow when the panel does.</summary>
    public Rect HoverRect { get; set; }

    /// <summary>Scale factor of the display the notch is on. GetCursorPos reports physical
    /// pixels while HoverRect is in DIP; this is the only place the two spaces meet.</summary>
    public double DpiScale { get; set; } = 1.0;

    /// <summary>True on entering the rectangle, false on leaving. Raised on transitions only.</summary>
    public event Action<bool>? HoverChanged;

    /// <summary>
    /// True when a drag that began elsewhere has reached the notch, false when it leaves or is
    /// released. Transitions only.
    ///
    /// This is readable at all only because GetCursorPos and GetAsyncKeyState are state reads
    /// rather than messages: the drag source owns the mouse for the duration and the notch
    /// receives no input of its own, but both calls keep answering. It is the single mechanism
    /// the shelf is built on — Plith's own window can see a drag coming and can never receive it.
    /// </summary>
    public event Action<bool>? DraggingOverChanged;

    /// <summary>
    /// The whole OSD window's screen rectangle in DIP — the open panel, not just the resting
    /// notch. Set by OsdHost after every reposition.
    /// </summary>
    public Rect PanelRect { get; set; }


    /// <summary>
    /// Whether the last poll found the cursor inside <see cref="PanelRect"/>.
    ///
    /// A polled property rather than an event, deliberately. The OSD is a WS_EX_LAYERED window
    /// with per-pixel alpha and Windows hit-tests layered windows against that alpha, so WPF's
    /// IsMouseOver is useless here: at rest the notch is a couple of opaque DIP in an otherwise
    /// transparent window, the cursor that triggers a hover is over transparent space, and the
    /// panel then opens beneath a stationary cursor that generates no further WM_MOUSEMOVE. On a
    /// live build IsMouseOver stayed false for the panel's entire life.
    ///
    /// An enter/leave event was tried and was also wrong: moving up toward the notch crosses the
    /// panel's rectangle before the resting shape's, so the enter transition fires while the
    /// notch is still parked, and a cursor that then stays put produces no second transition.
    /// A property the hide timer can consult at the moment it matters has neither problem.
    /// </summary>
    public bool IsCursorInPanel => _wasInsidePanel;

    /// <summary>
    /// Raised on every poll, not only on a transition.
    ///
    /// OsdHost uses it to re-derive the window's click-through bit from the presentation. That
    /// bit is otherwise only written on discrete events, several of which ride animation
    /// completion callbacks — and WPF raises no Completed for a clock that a competing animation
    /// replaced, which has produced four separate defects on this branch already. A dropped
    /// callback there leaves a parked, invisible notch still swallowing every click in a
    /// 440-DIP-wide band across the top of the screen, which is where people drag windows to
    /// maximise and reach browser tabs.
    ///
    /// Re-deriving costs a bool comparison per tick and cannot drift, because it recomputes the
    /// same expression the event handlers do rather than tracking a second copy of the answer.
    /// </summary>
    public event Action? Polled;

    public void Start()
    {
        _wasInside = false;
        _wasInsidePanel = false;
        _drag.Reset();

        // Fast, whatever rate the previous run ended on. A poller that restarted slow would
        // take its first look up to a fifth of a second after the notch appeared.
        _timer.Interval = NotchPollRate.Fast;
        _timer.Start();
    }

    public void Stop()
    {
        _timer.Stop();

        // Before the hover reset below, because a stop mid-drag has to put the notch back: the
        // OSD is hidden and the catcher is standing in its place, and nothing else would ever
        // tell either of them the drag is over.
        var wasApproaching = _drag.IsApproaching;
        _drag.Reset();
        if (wasApproaching) DraggingOverChanged?.Invoke(false);

        // Leave the world believing the cursor is outside, so a restart cannot open with a
        // stale "still inside" that never produces an enter transition.
        if (_wasInside)
        {
            _wasInside = false;
            HoverChanged?.Invoke(false);
        }
    }

    private void Poll()
    {
        if (!GetCursorPos(out var p)) return;

        var dip = NotchGeometry.PhysicalToDip(p.X, p.Y, DpiScale);

        _wasInsidePanel = NotchGeometry.IsInsideNotch(PanelRect, dip);

        Polled?.Invoke();

        var buttonDown = (GetAsyncKeyState(VK_LBUTTON) & 0x8000) != 0;

        // One line per press, which is the only volume at which this is affordable and the only
        // one that answers the question. A drag that produces no handoff is otherwise completely
        // silent, and "nothing happened" cannot distinguish a cursor that never entered the band
        // from a band that is in the wrong place, an empty rectangle, or a press the detector
        // read as starting on the OSD.
        if (buttonDown && !_buttonWasDownForLog)
        {
            _log?.Info("DragWatch",
                $"Press at {dip.X:0},{dip.Y:0} dip; hover={Describe(HoverRect)} " +
                $"band={Describe(NotchGeometry.DragApproachRect(HoverRect))} " +
                $"panel={Describe(PanelRect)} dpi={DpiScale:0.##}");
        }
        _buttonWasDownForLog = buttonDown;

        ApplyPollRate(dip, buttonDown);

        var inBand = NotchGeometry.IsInsideNotch(NotchGeometry.DragApproachRect(HoverRect), dip);

        // The other half of the question. The press line above says where a gesture began; this
        // says whether it ever arrived — and if it arrived and still produced no handoff, it
        // names the clause that rejected it. Without both, a drag that does nothing is the same
        // silence whether the cursor missed the band by a pixel or the origin test refused it.
        var heldInBand = buttonDown && inBand;
        if (heldInBand != _wasHeldInBandForLog)
        {
            _wasHeldInBandForLog = heldInBand;
            if (heldInBand)
            {
                _log?.Info("DragWatch",
                    $"Held cursor entered the band at {dip.X:0},{dip.Y:0}; " +
                    $"startedOutside={_drag.PressStartedOutside}");
            }
        }

        if (_drag.Update(buttonDown,
                         cursorOverOsd: _wasInsidePanel,
                         cursorInApproachBand: inBand,
                         cursorInHoldBand: NotchGeometry.IsInsideNotch(
                             NotchGeometry.DropTargetRect(HoverRect), dip)))
        {
            DraggingOverChanged?.Invoke(_drag.IsApproaching);
        }

        bool inside = NotchGeometry.IsInsideNotch(HoverRect, dip);

        if (inside == _wasInside) return;

        _wasInside = inside;
        HoverChanged?.Invoke(inside);
    }

    /// <summary>
    /// Re-rate the timer from where the cursor is now. See <see cref="NotchPollRate"/> for the
    /// rule and for what the flat 60 ms was costing.
    ///
    /// Guarded on inequality because assigning Interval restarts a DispatcherTimer's countdown:
    /// writing the same value every tick would keep pushing the next tick away.
    /// </summary>
    private void ApplyPollRate(Point cursorDip, bool buttonDown)
    {
        var want = NotchPollRate.For(PanelRect, cursorDip, buttonDown);
        if (_timer.Interval != want) _timer.Interval = want;
    }

    private bool _buttonWasDownForLog;
    private bool _wasHeldInBandForLog;

    private static string Describe(Rect r) =>
        r.IsEmpty || r.Width <= 0 ? "EMPTY" : $"{r.Left:0},{r.Top:0} {r.Width:0}x{r.Height:0}";

    public void Dispose() => _timer.Stop();

    [StructLayout(LayoutKind.Sequential)]
    private struct POINT { public int X, Y; }

    [DllImport("user32.dll")]
    private static extern bool GetCursorPos(out POINT lpPoint);

    private const int VK_LBUTTON = 0x01;

    /// <summary>A state read, not a message. That is why it still answers while a drag source
    /// owns the mouse and this window receives no input at all.</summary>
    [DllImport("user32.dll")]
    private static extern short GetAsyncKeyState(int vKey);
}
