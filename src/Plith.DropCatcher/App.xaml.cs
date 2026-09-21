using System.Globalization;
using System.Security.Principal;
using System.Windows;
using System.Windows.Threading;
using Plith.DropCatcher.Shelf;
using Plith.Services.Shelf;

namespace Plith.DropCatcher;

public partial class App : Application, IDisposable
{
    private CatcherLog _log = null!;
    private CatcherWindow _window = null!;
    private ShelfWindow _shelf = null!;
    private CatcherClient? _client;
    private DragSourceProbe? _probe;
    private Mutex? _single;

    protected override void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);

        _log = new CatcherLog();
        var sid = WindowsIdentity.GetCurrent().User?.Value ?? "unknown";

        // The single-instance guard is taken only in normal mode, and deliberately AFTER the
        // hand-run probe modes below. Both probes exist to be run alongside the real catcher -
        // the drag-out one has to run elevated, which is a second process by definition - and a
        // guard applied first would make them exit silently, which reads exactly like the thing
        // being measured having failed.
        if (e.Args.Length == 2 && e.Args[0].Equals("--dragout", StringComparison.OrdinalIgnoreCase))
        {
            // The other direction, and it is a MEASUREMENT rather than a feature. Dragging an
            // item out of the shelf means DoDragDrop from a High-integrity process to a Medium
            // one — the reverse of the direction already known to be blocked, and it may behave
            // differently: UIPI restricts what a LOWER integrity process may send to a higher
            // one, and here the higher one initiates. Run this elevated to stand in for an
            // installed Plith, drag from it into an Explorer window, and read the log.
            _log.Info($"DRAG-OUT PROBE. Integrity: {IntegrityLevel.Describe()}");
            var source = new DragOutWindow(_log, e.Args[1]);
            source.Show();
            return;
        }

        if (e.Args.Length == 2 && e.Args[0].Equals("--dragsource", StringComparison.OrdinalIgnoreCase))
        {
            // The follow-on question, and the last one the outbound direction needs answered
            // before it can be designed: the drag the catcher would have to start belongs to a
            // press that landed in ANOTHER process, because the tile the person pressed is
            // Plith's. Run this one at Medium — the level the catcher really runs at — and press
            // somewhere else. DragSourceProbe carries the whole explanation.
            _log.Info($"DRAG-SOURCE PROBE. Integrity: {IntegrityLevel.Describe()}");
            _probe = new DragSourceProbe(_log, e.Args[1]);
            _probe.Start();
            return;
        }

        if (TryReadRect(e.Args, "--shelfprobe", out var shelfRect))
        {
            // The shelf, driven by hand, before anything on Plith's side depends on it.
            //
            // It is a probe rather than a test because nothing in the headless suite can host a
            // window, not because nothing can look at one. THE CLAIM THAT USED TO STAND HERE -
            // that a layered window cannot be captured over Remote Desktop by any means, so only
            // a person at the physical console could check this - WAS WRONG, and was never
            // measured against this window: it was inherited from the OSD's notes, and the OSD is
            // a different process at a different integrity level. Two instruments drive this mode
            // now. scripts/capture-shelf.ps1 captures the shelf from the screen, over RDP, and
            // scripts/drive-shelf.ps1 presses it: this window is MEDIUM integrity (the log line
            // below says so on every run), so UIPI does not stand between it and synthetic input
            // or UI Automation the way it does for Plith's own UIAccess window.
            // docs/SHELF-VERIFICATION.md section 3.10 carries the measurements and the limits.
            _log.Info($"SHELF PROBE. Integrity: {IntegrityLevel.Describe()}. Log: {_log.LogPath}");
            _shelf = new ShelfWindow(_log);
            WireShelf(_shelf);
            _shelf.Dismissed += () =>
            {
                _log.Info("SHELF PROBE: the shelf reported itself closed. Exiting.");
                Shutdown();
            };

            // Two stacks rather than one, so a stack separator is in the picture, and three
            // entries rather than two, so one column carries two tiles and the other carries one:
            // a page with every column the same shape cannot show a column that lines up wrongly.
            // The paths are invented and need not exist. ShelfModel stats them only to choose
            // between the folder icon and the document icon, and a path that is neither is drawn
            // as a document, which is what these three are meant to be.
            _shelf.SetItems(["C:\\Probe\\quarterly-report.pdf", "C:\\Probe\\screenshot.png",
                             "C:\\Probe\\invoice-2026-09.xlsx"]);
            _shelf.OpenAt(shelfRect.x, shelfRect.y, shelfRect.w, shelfRect.h);
            return;
        }

        _window = new CatcherWindow(_log);
        _window.FilesDropped += OnFilesDropped;
        _window.Withdrew += OnWithdrew;

        if (TryReadProbeRect(e.Args, out var probe))
        {
            _log.Info($"Started. Integrity: {IntegrityLevel.Describe()}");
            // Stand-alone mode, for measuring whether a Medium window can receive a drop at all
            // without Plith having to be involved. Nothing else in the design is worth building
            // if this fails, so it is reachable on its own.
            _log.Info($"PROBE MODE. Log: {_log.LogPath}");
            _window.ShowAt(probe.x, probe.y, probe.w, probe.h);
            return;
        }

        // After the catcher probe returns, not before it. Constructing a ShelfWindow builds its
        // visual tree, applies a palette and renders the page, and the catcher probe never shows
        // a shelf: doing that work above would put a second window's worth of layout into the one
        // mode that exists to measure the first window on its own.
        _shelf = new ShelfWindow(_log);
        WireShelf(_shelf);
        _shelf.Dismissed += () => _ = _client?.SendAsync(new DropMessage(DropVerb.ShelfClosed, 0, 0, 0, 0, []));

        // Per-user, matching the pipe name: one catcher per signed-in session, and a second
        // instance quits rather than fighting the first for the same rectangle.
        _single = new Mutex(true, @"Local\Plith.DropCatcher." + sid, out var isFirst);
        if (!isFirst)
        {
            _log.Info("Another catcher is already running for this user. Exiting.");
            Shutdown();
            return;
        }

        _log.Info($"Started. Integrity: {IntegrityLevel.Describe()}");

        _client = new CatcherClient(sid, _log);
        _client.Received += OnReceived;
        _client.Start();

        // Warmed here, at startup, while nobody is waiting for anything. See ShelfWindow.Warm for
        // the measurement: without it the FIRST open of the shelf costs about 182 ms of WPF
        // creating its first window in this process, against 12 ms for every open after it, and
        // for those 182 ms the busy cursor is what a person sees.
        //
        // Deferred one dispatcher turn rather than called straight away, so the pipe's own
        // startup is not sharing a frame with a layout pass.
        // Background, not ApplicationIdle. Measured: at ApplicationIdle the callback never ran at
        // all in this process, and a warm-up that does not happen is worse than none, because the
        // log then claims a cost has been paid that has not. Background runs after the startup
        // work and before anything a person can trigger.
        Dispatcher.BeginInvoke(new Action(() =>
        {
            try { _shelf.Warm(); }
            catch (Exception ex)
            {
                // Logged rather than swallowed and rather than allowed to kill the process: the
                // warm-up is an optimisation, and the shelf works without it at the cost of one
                // slow first open.
                _log.Info($"Warm-up failed and was skipped: {ex.GetType().Name}: {ex.Message} at {ex.StackTrace}");
            }
        }), DispatcherPriority.Background);
    }

    /// <summary>
    /// A message from Plith.
    ///
    /// WRAPPED, and the wrapper is not defensive programming for its own sake: it is here because
    /// the absence of it cost a whole hardware run and pointed at the wrong thing. A new verb's
    /// handler asked for a resource key this project does not define, FindResource threw, and the
    /// throw travelled out of this Invoke and took the OpenShelf message that came after it with
    /// it. What the catcher's log then showed was an Items line and nothing else: the shelf never
    /// appeared, with no error anywhere, and the run blamed the pipe.
    ///
    /// One message failing must cost that message and nothing more. Logged rather than swallowed,
    /// because a verb that always throws would otherwise be invisible forever.
    /// </summary>
    private void OnReceived(DropMessage message) => Dispatcher.Invoke(() =>
    {
        try { Route(message); }
        catch (Exception ex)
        {
            _log.Info($"Message {message.Verb} failed and was dropped: {ex.GetType().Name}: {ex.Message}");
        }
    }, DispatcherPriority.Send);

    private void Route(DropMessage message)
    {
        switch (message.Verb)
        {
            case DropVerb.Show:
                _window.ShowAt((int)message.X, (int)message.Y, (int)message.W, (int)message.H);
                break;
            case DropVerb.Hide:
                HideStandIn();
                break;
            case DropVerb.OpenShelf:
                _shelf.OpenAt((int)message.X, (int)message.Y, (int)message.W, (int)message.H);

                // Sent the moment the window is up, so Plith can take its own down without
                // leaving a gap between the two. OpenAt has already called Show and placed the
                // window by the time it returns, so this is not a promise about a future frame:
                // the surface is on screen.
                _ = _client?.SendAsync(new DropMessage(DropVerb.ShelfShown, 0, 0, 0, 0, []));

                // Opened by a drop, rather than by paging to the shelf: hold it on screen, or the
                // pointer that is already leaving takes it down within a second.
                if (DateTime.UtcNow - _droppedAt < DropWindow)
                {
                    _droppedAt = DateTime.MinValue;
                    _shelf.HoldOpen(DropHold);
                    _log.Info($"Shelf held for {DropHold.TotalMilliseconds:0} ms: it is a drop's acknowledgement.");
                }
                break;
            case DropVerb.Items:
                // One message is the whole shelf now, so X and Y carry nothing and are ignored.
                _shelf.SetItems(message.Paths);
                break;
            case DropVerb.CloseShelf:
                // Plith paged away from the shelf. CloseNow refuses while a drag is in flight and
                // remembers the order, which is why this is a send rather than a request.
                _shelf.CloseNow();
                break;
            case DropVerb.Rail:
                // How many pages there are and which one is the shelf. Neither is knowable here.
                _shelf.SetRail((int)message.X, (int)message.Y);
                break;
            case DropVerb.Palette:
                if (ShelfPaletteWire.TryFromPaths(message.Paths, out var palette)) _shelf.Apply(palette);
                else _log.Info("Palette payload did not decode; keeping the built-in colours.");
                break;
            default:
                break;
        }
    }

    /// <summary>
    /// Take the stand-in down, and do it AFTER the shelf has finished arriving if the shelf is
    /// what is replacing it.
    ///
    /// Both windows belong to this process and sit on the same rectangle, and both are layered
    /// with per-pixel alpha: while the shelf fades in, whatever is behind it shows through. With
    /// the pill hidden first that is the DESKTOP, which is a flash of nothing in the middle of a
    /// drop. Left underneath, it is the pill, so the two read as one surface changing into
    /// another.
    ///
    /// Only for the drop path in practice: a withdrawal with no shelf behind it takes the pill
    /// down at once, which is what IsOpen distinguishes.
    /// </summary>
    private void HideStandIn()
    {
        if (!_shelf.IsOpen)
        {
            _window.HideNow();
            return;
        }

        // One timer, restarted, rather than one per call: two Hides in quick succession would
        // otherwise leave two clocks running and hide the pill twice, and the second could land
        // after the pill has been shown again for the next drag.
        _standInHide ??= new DispatcherTimer(DispatcherPriority.Send, Dispatcher);
        _standInHide.Interval = Shelf.ShelfWindow.FadeIn;
        _standInHide.Tick -= OnStandInHideElapsed;
        _standInHide.Tick += OnStandInHideElapsed;
        _standInHide.Stop();
        _standInHide.Start();
    }

    private DispatcherTimer? _standInHide;

    private void OnStandInHideElapsed(object? sender, EventArgs e)
    {
        _standInHide?.Stop();
        _window.HideNow();
    }

    /// <summary>Tell Plith to put the notch back. Hide is the verb in this direction too, and the
    /// two ends distinguish them by who sent it.</summary>
    private void OnWithdrew()
        => _ = _client?.SendAsync(new DropMessage(DropVerb.Hide, 0, 0, 0, 0, []));

    private void OnFilesDropped(IReadOnlyList<string> paths)
    {
        // Remembered, because the shelf that opens a moment from now is this drop's
        // acknowledgement and has to stay long enough to be one. Plith could send the fact, but
        // it is already known HERE: this process is the one that caught the files.
        _droppedAt = DateTime.UtcNow;
        _ = _client?.SendAsync(new DropMessage(DropVerb.Dropped, 0, 0, 0, 0, paths));
    }

    /// <summary>When this process last caught a drop. See <see cref="OnFilesDropped"/>.</summary>
    private DateTime _droppedAt = DateTime.MinValue;

    /// <summary>
    /// How soon after a drop an OpenShelf counts as that drop's acknowledgement.
    ///
    /// Generous, because the round trip in between is a pipe write, a store write and a window
    /// swap, and mean, because a person who pages to the shelf a second later has not asked for
    /// anything to be held.
    /// </summary>
    private static readonly TimeSpan DropWindow = TimeSpan.FromMilliseconds(1200);

    /// <summary>How long a drop's acknowledgement stays on screen. The 2.6 seconds Plith's own
    /// landing page used, less the fade, so the two feel the same.</summary>
    private static readonly TimeSpan DropHold = TimeSpan.FromMilliseconds(2400);

    /// <summary>
    /// Bridges the shelf's four wire-bound requests onto the client, the same way FilesDropped
    /// and Withdrew are bridged above. Called from both places a ShelfWindow is constructed
    /// (the real run and the hand-run shelf probe) rather than once, since neither construction
    /// site shares a common caller. In the probe, _client is null and every send below is a
    /// silent no-op through the null-conditional operator: correct, because the probe has no
    /// Plith on the other end of a pipe to answer it, and a shelf whose remove or restack
    /// visibly did nothing is exactly the reminder that this mode has no wire, not a bug in it.
    /// </summary>
    private void WireShelf(ShelfWindow shelf)
    {
        shelf.ClearShelfRequested += () =>
            _ = _client?.SendAsync(new DropMessage(DropVerb.ClearShelf, 0, 0, 0, 0, []));
        shelf.RemoveItemsRequested += paths =>
            _ = _client?.SendAsync(new DropMessage(DropVerb.RemoveItems, 0, 0, 0, 0, paths));
        // A paging gesture: a wheel delta or a page index, with the other zero. Carried rather
        // than acted on, because the pager lives in Plith and there is one of it.
        shelf.PageRequested += (delta, index) =>
            _ = _client?.SendAsync(new DropMessage(DropVerb.Page, delta, index, 0, 0, []));
    }

    private static bool TryReadProbeRect(string[] args, out (int x, int y, int w, int h) rect)
        => TryReadRect(args, "--probe", out rect);

    /// <summary>Reads "&lt;flag&gt; x y w h" in physical screen pixels, the same four numbers the
    /// wire carries, so a probe puts a window exactly where a real message would.</summary>
    private static bool TryReadRect(string[] args, string flag, out (int x, int y, int w, int h) rect)
    {
        rect = default;
        if (args.Length != 5 || !args[0].Equals(flag, StringComparison.OrdinalIgnoreCase)) return false;

        var numbers = new int[4];
        for (var i = 0; i < 4; i++)
        {
            if (!int.TryParse(args[i + 1], NumberStyles.Integer, CultureInfo.InvariantCulture, out numbers[i]))
                return false;
        }

        rect = (numbers[0], numbers[1], numbers[2], numbers[3]);
        return true;
    }

    protected override void OnExit(ExitEventArgs e)
    {
        Dispose();
        base.OnExit(e);
    }

    public void Dispose()
    {
        _client?.Dispose();
        _client = null;
        _probe?.Dispose();
        _probe = null;
        _single?.Dispose();
        _single = null;
        GC.SuppressFinalize(this);
    }
}
