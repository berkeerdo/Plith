using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Media;
using Plith.Views.Presentation;

namespace Plith.Services.Shelf;

/// <summary>
/// The open-and-close conversation with the drop catcher.
///
/// Separate from OsdHost because OsdHost is a window and this is a protocol. Everything here is
/// a request to another process and an answer that may never come, and mixing that with a
/// window's layout is how both become hard to follow.
///
/// Every message arriving here came from a process at a lower integrity level over a pipe any
/// process on this machine can write. ShelfStore is the only thing that decides what is true;
/// this class routes, and routes nothing that ShelfStore would not check.
///
/// Not thread-safe, and deliberately not made so: every method must be called on the UI thread,
/// because ShelfStore is not thread-safe either and the pages that repaint from its Changed
/// event are WPF controls. The pipe's read loop runs off the UI thread, so whoever forwards a
/// message to <see cref="HandleMessage"/> is the one that has to marshal.
/// </summary>
public sealed class ShelfSession
{
    /// <summary>
    /// Hands this process's right to set the foreground window to another one, named by process
    /// id. The documented way to let a process you are about to ask for a window actually show
    /// it focused.
    ///
    /// It is needed because of a measured failure, written up in docs/SHELF-VERIFICATION.md: with
    /// another application holding the foreground, the catcher's Activate() fails, the shelf logs
    /// foreground=False, Esc does nothing and Deactivated never fires. The shelf is then on
    /// screen and dismissable only by the mouse-leave timer, which looks exactly like the shelf
    /// being broken.
    /// </summary>
    [DllImport("user32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool AllowSetForegroundWindow(int dwProcessId);

    private readonly DropChannelServer _channel;
    private readonly ShelfStore _store;
    private readonly Func<ShelfPalette> _palette;
    private readonly DiagnosticLog? _log;

    /// <param name="palette">Asked for at every open rather than captured once. The accent and
    /// the theme both change while the app runs, and a palette resolved at construction would
    /// paint the shelf in whatever was true when Plith started.</param>
    public ShelfSession(DropChannelServer channel, ShelfStore store, Func<ShelfPalette> palette,
                        DiagnosticLog? log = null)
    {
        _channel = channel;
        _store = store;
        _palette = palette;
        _log = log;
    }

    /// <summary>The catcher has been asked to show the shelf. Raised before it has done so:
    /// there is no acknowledgement on the wire, and waiting for one that does not exist would
    /// mean the notch staying up over a shelf that is already growing.</summary>
    public event Action? Opened;

    /// <summary>The shelf reported itself gone.</summary>
    public event Action? Closed;

    /// <summary>The shelf cannot be shown, with a sentence saying why. A click that does nothing
    /// is indistinguishable from the product being broken, so this always carries something a
    /// person can read.</summary>
    public event Action<string>? Unavailable;

    /// <summary>
    /// Ask the catcher for the shelf, in the notch's place.
    ///
    /// <paramref name="notchRectDip"/> is the notch's hover rectangle, the same input
    /// <see cref="NotchGeometry.DropTargetRect"/> takes, so the shelf lands centred on exactly
    /// the anchor the open frame it grows out of was centred on.
    ///
    /// Palette first, then the items, then OpenShelf. The surface must have everything it needs
    /// before it is told to appear, or the person watches it assemble itself: the shape grows
    /// holding the built-in fallback colours and an empty page, and repaints once the rest
    /// catches up.
    /// </summary>
    public void Open(Rect notchRectDip, double dpiScale)
    {
        var start = DropCatcherLauncher.EnsureRunning(_log);

        if (!_channel.IsConnected)
        {
            var why = Explain(start);
            _log?.Info("Shelf", $"Shelf requested but no catcher is connected ({start}).");
            Unavailable?.Invoke(why);
            return;
        }

        GrantForeground();

        var (x, y, w, h) = NotchGeometry.DipToPhysical(NotchGeometry.ShelfRect(notchRectDip), dpiScale);

        Send(DropVerb.Palette, ShelfPaletteWire.ToPaths(_palette()));
        SendStacks();
        Send(DropVerb.OpenShelf, [], x, y, w, h);

        _log?.Info("Shelf", $"Shelf requested at {x},{y} {w}x{h} with {_store.Stacks.Count} stack(s).");
        Opened?.Invoke();
    }

    /// <summary>
    /// Something the catcher asked for.
    ///
    /// Every verb below changes the SHELF, never this process: there is no verb here that runs
    /// anything, opens anything or touches a file. That is the whole shape of the trust boundary
    /// — the catcher describes what the person did to the surface, and ShelfStore decides what
    /// that means, including refusing paths it cannot stat and indices out of range.
    /// </summary>
    public void HandleMessage(DropMessage message)
    {
        switch (message.Verb)
        {
            case DropVerb.RemoveItems:
                _store.RemoveMany(message.Paths);
                SendStacks();
                break;

            case DropVerb.ClearShelf:
                _store.Clear();
                SendStacks();
                break;

            case DropVerb.NewStack:
                _store.NewStack();
                SendStacks();
                break;

            case DropVerb.Restack:
                // X is the target index, carried as a double like every other number on the wire.
                // Out of range is ShelfStore's decision, not this one: it is a claim from another
                // process like the paths beside it.
                _store.Restack((int)message.X, message.Paths);
                SendStacks();
                break;

            case DropVerb.ShelfClosed:
                // Pruned HERE rather than when a stack empties, because an empty stack is a live
                // thing while the shelf is up: NewStack makes one on purpose so the next drop has
                // somewhere of its own to go, and removing the last item from a stack must not
                // make the stack vanish under the pointer. Once the surface is gone there is
                // nothing to hold a place for.
                _store.PruneEmptyStacks();
                Closed?.Invoke();
                break;

            default:
                // Show, Hide, Hello and Dropped belong to the drag path and are answered in App.
                break;
        }
    }

    /// <summary>
    /// The whole shelf, one message per stack.
    ///
    /// Re-sent after every change rather than diffed, because the catcher's model is a VIEW: it
    /// holds no state Plith does not, so the cheapest correct refresh is to say what is true now.
    /// A shelf holds at most twenty items, so "cheapest" is not a figure of speech.
    ///
    /// An EMPTY shelf still sends one message, and that is not a formality. The obvious loop
    /// sends nothing when there are no stacks, which is silence — and silence on this wire means
    /// "nothing changed", so clearing the shelf while it is open would leave every tile on
    /// screen. ShelfModel reads index 0 as the start of a fresh delivery and a total of 0 as a
    /// delivery with no stacks in it, so one message saying (0, 0, no paths) is how "there is
    /// nothing here now" is spelled.
    /// </summary>
    private void SendStacks()
    {
        var stacks = _store.Stacks;
        if (stacks.Count == 0)
        {
            Send(DropVerb.Items, [], x: 0, y: 0);
            return;
        }

        for (var i = 0; i < stacks.Count; i++)
        {
            Send(DropVerb.Items, stacks[i].Select(item => item.Path).ToList(),
                 x: i, y: stacks.Count);
        }
    }

    /// <summary>
    /// Fire and forget, and safe to do so only because DropChannelServer serializes its sends.
    /// Several Items messages are in flight here at once by construction, which is precisely the
    /// case where two writers on one stream could interleave their bytes mid-line. See
    /// DropChannelServer.SendAsync.
    /// </summary>
    private void Send(DropVerb verb, IReadOnlyList<string> paths,
                      double x = 0, double y = 0, double w = 0, double h = 0)
        => _ = _channel.SendAsync(new DropMessage(verb, x, y, w, h, paths));

    private void GrantForeground()
    {
        var pid = DropCatcherLauncher.FindProcessId();
        if (pid is null)
        {
            // Connected but not found by name. Possible if the catcher exited between the pipe's
            // last write and this call, and worth a line rather than a silent skip: the shelf
            // that follows will not be focusable and nobody would know why.
            _log?.Warn("Shelf", "Cannot find the catcher's process id; foreground not handed over.");
            return;
        }

        // Logged either way. The grant fails silently by design — the call returns FALSE when
        // this process does not itself hold the foreground right, and Plith's OSD window is
        // never activated, so that is a real possibility rather than a theoretical one. A shelf
        // that opens unfocused is indistinguishable on screen from one that opens focused, and
        // only becomes visible as Esc not working, so the two halves of the answer live in two
        // logs: this line, and the catcher's own foreground= on the line it writes when it opens.
        var granted = AllowSetForegroundWindow(pid.Value);
        _log?.Info("Shelf", $"AllowSetForegroundWindow(pid {pid.Value}) returned {granted}.");
    }

    private static string Explain(CatcherStart start) => start switch
    {
        // Two different waits, and they need different sentences: one has just been started and
        // will be along, the other was already running and therefore is NOT along — it is up but
        // has not reached the pipe, which is what a catcher still starting looks like.
        CatcherStart.Started => "Starting the shelf helper. Try again in a moment.",
        CatcherStart.AlreadyRunning => "The shelf helper is starting up. Try again in a moment.",
        CatcherStart.NotFound => "The shelf helper is missing from this install.",
        CatcherStart.Failed => "Windows would not start the shelf helper.",
        _ => "The shelf is not available.",
    };

    /// <summary>
    /// The theme, resolved the way the running notch resolves it.
    ///
    /// Every value here is taken from the same call ThemeService.BuildAccentOverride makes for
    /// the key beside it, rather than being derived a second way: NotchInk is PairOn(SurfaceEnd)
    /// there and is PairOn(SurfaceEnd) here, and so on down the list. A second derivation would
    /// drift, and the drift would show as the shelf not looking like the product it belongs to
    /// — which is the whole reason the ANSWER crosses the wire rather than the accent.
    ///
    /// The two surfaces are sent WITHOUT the F0 alpha ThemeService applies. The catcher paints
    /// them on its own layered window with its own opacity, and an alpha applied twice would
    /// compound.
    /// </summary>
    public static ShelfPalette DerivePalette(Color baseColor, bool isDark)
    {
        var accent = AccentTheme.Derive(baseColor, isDark).Accent;
        var surfaces = AccentTheme.DeriveOsdSurfaces(baseColor, isDark);
        var ink = ContrastInk.PairOn(surfaces.SurfaceEnd);

        return new ShelfPalette(
            SurfaceStart: surfaces.SurfaceStart,
            SurfaceEnd: surfaces.SurfaceEnd,
            Ink: ink.Ink,
            InkMuted: ink.Muted,
            Track: ContrastInk.TrackOn(surfaces.SurfaceEnd),
            Accent: accent,
            // The DERIVED ring, never the raw accent. RingOn returns the accent untouched when it
            // already clears 3:1 against the surface and walks its lightness when it does not; a
            // near-white accent measured 1.25:1 sent raw.
            SelectionRing: ContrastInk.RingOn(accent, surfaces.SurfaceEnd),
            IsDark: isDark);
    }
}
