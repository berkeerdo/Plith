using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Threading;

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
/// The timer runs for as long as notch mode is the active presentation — including while the
/// panel is open, since the resting rectangle's own hover state still needs tracking to know
/// when the user has left it — and stops only when the mode switches away from the notch, so Classic
/// pays nothing for this.
/// </summary>
internal sealed class NotchHoverPoller : IDisposable
{
    private static readonly TimeSpan Interval = TimeSpan.FromMilliseconds(60);

    private readonly DispatcherTimer _timer;
    private bool _wasInside;
    private bool _wasInsidePanel;

    public NotchHoverPoller(Dispatcher dispatcher)
    {
        _timer = new DispatcherTimer(DispatcherPriority.Background, dispatcher) { Interval = Interval };
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
        _timer.Start();
    }

    public void Stop()
    {
        _timer.Stop();
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

        bool inside = NotchGeometry.IsInsideNotch(HoverRect, dip);
        if (inside == _wasInside) return;

        _wasInside = inside;
        HoverChanged?.Invoke(inside);
    }

    public void Dispose() => _timer.Stop();

    [StructLayout(LayoutKind.Sequential)]
    private struct POINT { public int X, Y; }

    [DllImport("user32.dll")]
    private static extern bool GetCursorPos(out POINT lpPoint);
}
