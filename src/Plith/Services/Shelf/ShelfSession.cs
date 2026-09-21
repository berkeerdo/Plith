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
/// <summary>
/// Why the shelf went away, which decides what the notch does next.
///
/// THE TWO CAUSES WANT OPPOSITE THINGS, and treating them alike is a defect a person reported
/// twice: "when I try to scroll to the other widgets the notch closes".
/// </summary>
public enum ShelfCloseCause
{
    /// <summary>
    /// The person paged to another widget, so Plith asked for the shelf to go.
    ///
    /// The frame stays OPEN: they are still reading the notch and the next page is what they
    /// asked for. Parking here collapses the whole notch on the way past the shelf, which is what
    /// it did.
    /// </summary>
    PageTurn,

    /// <summary>
    /// The surface itself ended: the pointer left it, Esc, another window took focus, or the
    /// catcher died.
    ///
    /// The frame goes to REST, because the person is done with the notch rather than moving
    /// through it. Without this the catcher's shelf disappears and Plith's own page takes its
    /// place for whatever the hide timer has left.
    /// </summary>
    Surface,
}

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

    /// <summary>
    /// A paging gesture happened on the shelf page: a raw wheel delta, or a page index, with the
    /// other zero. Raised on the UI thread, because App marshals every message before routing it.
    /// </summary>
    public event Action<int, int>? PageRequested;

    /// <summary>
    /// The shelf's window is on screen. Plith's own window can go down now, with nothing visible
    /// between the two.
    ///
    /// Distinct from <see cref="Opened"/>, which fires when the REQUEST goes out and is what the
    /// notch's stand-aside bookkeeping keys on. This one is the catcher answering.
    /// </summary>
    public event Action? Shown;

    /// <summary>The catcher has been asked to show the shelf. Raised before it has done so:
    /// there is no acknowledgement on the wire, and waiting for one that does not exist would
    /// mean the notch staying up over a shelf that is already growing.</summary>
    public event Action? Opened;

    /// <summary>
    /// The shelf is gone, whichever way it went.
    ///
    /// TWO causes, and the second is the one that matters. Usually the surface reported itself
    /// closed. But the catcher can also die while the shelf is up, and then no ShelfClosed ever
    /// arrives: Plith's own window is hidden for the duration, so without a second cause the
    /// result is not a missing notch, it is a missing OSD, with volume keys showing nothing until
    /// Plith restarts. <see cref="OnChannelLost"/> is that second cause.
    /// </summary>
    public event Action<ShelfCloseCause>? Closed;

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
    /// <summary>
    /// How many widget pages there are, and which one is the shelf, so the catcher can draw the
    /// notch's own rail while it holds the frame.
    ///
    /// Set by OsdHost, which owns the pager, rather than read from anything here: the page list
    /// follows settings (the weather page comes and goes) and this class has no business knowing
    /// that. Zero means "do not draw a rail", which is what a caller that never set it gets.
    /// </summary>
    public int RailPageCount { get; set; }

    /// <inheritdoc cref="RailPageCount"/>
    public int RailShelfIndex { get; set; }

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

        // The NOTCH'S OWN FRAME, not a rectangle of the shelf's own. ShelfPageRect is
        // DropTargetRect, and the item count is no longer an input: the shelf is a page in the
        // frame rather than a pane that hugs its contents.
        var (x, y, w, h) = NotchGeometry.DipToPhysical(NotchGeometry.ShelfPageRect(notchRectDip), dpiScale);

        Send(DropVerb.Palette, ShelfPaletteWire.ToPaths(_palette()));
        SendItems();
        Send(DropVerb.Rail, [], RailPageCount, RailShelfIndex);
        Send(DropVerb.OpenShelf, [], x, y, w, h);

        // Asked AGAIN, after the sends. The first check can be stale by the time it matters: the
        // read loop runs on its own thread and may have noticed the catcher was gone while this
        // method was resolving a palette. Reporting Opened anyway takes the notch down for a
        // shelf that will never appear and never close, which is the worst outcome this method
        // has. It does not close the window fully, because PipeStream caches its state rather
        // than probing it, so a catcher killed a microsecond ago still reads as connected here;
        // that case is answered by OnChannelLost instead, which is why both exist.
        if (!_channel.IsConnected)
        {
            _log?.Warn("Shelf", "The catcher went away while the shelf was being sent.");
            Unavailable?.Invoke("The shelf helper stopped responding.");
            return;
        }

        _shelfOpen = true;
        _log?.Info("Shelf", $"Shelf requested at {x},{y} {w}x{h} with {_store.Items.Count} item(s).");
        Opened?.Invoke();
    }

    /// <summary>
    /// Take the shelf down: the page turned away from it, or the frame collapsed.
    ///
    /// Raises <see cref="Closed"/> immediately rather than waiting for the catcher to answer with
    /// ShelfClosed, for the same reason <see cref="Open"/> raises Opened before the shelf is on
    /// screen: there is no acknowledgement on this wire, and waiting for one that may never come
    /// would leave Plith's own window hidden behind a shelf that is already going away.
    ///
    /// A ShelfClosed that arrives afterwards is harmless: the guard below makes this idempotent
    /// and HandleMessage's branch does the same thing again to a session that is already closed.
    /// </summary>
    public void Close()
    {
        if (!_shelfOpen) return;

        _shelfOpen = false;
        Send(DropVerb.CloseShelf, []);
        _log?.Info("Shelf", "Shelf closed by Plith: the page turned away from it.");
        Closed?.Invoke(ShelfCloseCause.PageTurn);
    }

    /// <summary>
    /// The catcher is gone and a shelf was up. Put the notch back.
    ///
    /// Called from the channel's Disconnected signal, marshalled onto the UI thread by App. Does
    /// NOT prune empty stacks, unlike an orderly ShelfClosed: pruning is a tidy-up for a surface
    /// that finished its work, and this surface did not. A stack the person made and had not yet
    /// filled survives to the next open rather than being swept away because a process crashed.
    /// </summary>
    public void OnChannelLost()
    {
        if (!_shelfOpen) return;

        _shelfOpen = false;
        _log?.Warn("Shelf", "The catcher went away while the shelf was open; putting the notch back.");
        Closed?.Invoke(ShelfCloseCause.Surface);
    }

    /// <summary>Whether a shelf is believed to be on screen. Believed rather than known: the only
    /// evidence is what has been sent and what has come back, and the whole point of
    /// <see cref="OnChannelLost"/> is that the answer can stop being true without anyone saying
    /// so.</summary>
    private bool _shelfOpen;

    /// <summary>
    /// Something the catcher asked for.
    ///
    /// Every verb below changes the SHELF, never this process: there is no verb here that runs
    /// anything, opens anything or touches a file. That is the whole shape of the trust
    /// boundary. The catcher describes what the person did to the surface, and ShelfStore decides
    /// what that means, including refusing paths it cannot stat and indices out of range.
    /// </summary>
    public void HandleMessage(DropMessage message)
    {
        switch (message.Verb)
        {
            case DropVerb.RemoveItems:
            {
                // Counted on BOTH sides of the call, because a remove that kept nothing and a
                // remove that was never asked for look identical in a log that only says "after".
                var before = _store.Items.Count;
                _store.RemoveMany(message.Paths);
                _log?.Info("Shelf", $"RemoveItems({message.Paths.Count}): {before} -> {_store.Items.Count}.");
                SendItems();
                break;
            }

            case DropVerb.ClearShelf:
            {
                var before = _store.Items.Count;
                _store.Clear();
                _log?.Info("Shelf", $"ClearShelf: {before} -> {_store.Items.Count}.");
                SendItems();
                break;
            }

            case DropVerb.ShelfClosed:
                _shelfOpen = false;
                Closed?.Invoke(ShelfCloseCause.Surface);
                break;

            case DropVerb.ShelfShown:
                Shown?.Invoke();
                break;

            case DropVerb.Page:
                // Routed, not interpreted. The numbers arrive from a lower-integrity process, so
                // they are a REQUEST: OsdHost feeds them to the same pager every other page turn
                // goes through, which is what clamps an index and what decides whether a delta is
                // a commit at all.
                PageRequested?.Invoke((int)message.X, (int)message.Y);
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
    /// sends nothing when there are no stacks, which is silence, and silence on this wire means
    /// "nothing changed", so clearing the shelf while it is open would leave every tile on
    /// screen. ShelfModel reads index 0 as the start of a fresh delivery and a total of 0 as a
    /// delivery with no stacks in it, so one message saying (0, 0, no paths) is how "there is
    /// nothing here now" is spelled.
    /// </summary>
    /// <summary>
    /// The whole shelf in one message.
    ///
    /// It used to be one message PER STACK, carrying the stack's index and the total so the
    /// catcher could reassemble them. That reassembly was the most intricate code in the shelf,
    /// and all of it existed to survive two deliveries interleaving on a pipe any local process
    /// may write. One flat list is one message, so there is nothing to assemble and nothing to
    /// interleave with itself.
    ///
    /// An empty shelf is a message too: the surface must be TOLD it is empty, not left holding
    /// what it had.
    /// </summary>
    private void SendItems()
    {
        var paths = _store.Items.Select(item => item.Path).ToArray();
        _log?.Info("Shelf", $"Items -> catcher: {paths.Length} path(s).");
        Send(DropVerb.Items, paths, x: 0, y: 0);
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

        // Logged either way. The grant fails silently by design: the call returns FALSE when
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
        // will be along, the other was already running and therefore is NOT along: it is up but
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
    /// drift, and the drift would show as the shelf not looking like the product it belongs to,
    /// which is the whole reason the ANSWER crosses the wire rather than the accent.
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
