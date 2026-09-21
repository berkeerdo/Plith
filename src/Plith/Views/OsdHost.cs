using System.Globalization;
using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Threading;
using Plith.Cards;
using Plith.Interop;
using Plith.Services;
using Plith.ViewModels;
using Plith.Views.Presentation;
using Plith.Views.Widgets;
using WpfScreenHelper;

using Plith.Services.Shelf;

namespace Plith.Views;

/// <summary>
/// BandWindow-backed OSD host. Creates a native HWND via CreateWindowInBand in the
/// highest z-band the current process is allowed to enter (UIAccess when granted,
/// Desktop otherwise). Replaces the Phase 1 OsdWindow : Window approach, which
/// could not draw above exclusive fullscreen games.
/// </summary>
// CA1001: OsdHost owns _hoverPoller (IDisposable). It's released in the Application.Exit
// handler wired up below, the same pattern BandWindow itself uses for _hwndSource — see
// the SuppressMessage on BandWindow for why this class doesn't implement IDisposable either.
[System.Diagnostics.CodeAnalysis.SuppressMessage("Design", "CA1001:Types that own disposable fields should be disposable",
    Justification = "_hoverPoller is released in the Application.Exit handler wired in the constructor; WPF visual tree owns the rest of the lifecycle.")]
public sealed class OsdHost : BandWindow
{
    // Magnet radius: while dragging or free-clicking, the OSD's center snaps to the
    // nearest 3x3 hotspot when it's within this many DIPs. Roomy enough for a strong
    // "pull" feel without preventing fine placement (Alt bypasses it completely).
    private const double SnapThresholdDip = 80;

    private readonly OsdContent _content;
    private readonly SettingsService _settings;
    private readonly ThemeService _theme;
    private readonly CardHost _cardHost;
    private readonly NotchHomeState _home;
    private readonly DiagnosticLog? _log = new();
    private IOsdPresentation _presentation;
    private readonly NotchHoverPoller _hoverPoller;

    // The widget pager. Owns the page index and the wheel-delta accumulator, and owns them
    // outside any animation or callback - see NotchPager for why that is not incidental.
    // The page count is a placeholder until the widget frame exists and can report its own.
    private readonly NotchPager _pager = new(1);

    // Built once and kept for the life of the host, not per open. Its pages own timers and
    // storyboards that are started and stopped by visibility, so rebuilding them on every open
    // would churn exactly the resources this slice is trying to keep bounded.
    private readonly Widgets.WidgetFrame _widgets = new();

    // Built when the audio and media view models arrive, for the same reason the frame's pages
    // are: it reads them live, and a HUD rebuilt per event would resubscribe every time.
    private Widgets.NotchHud? _hud;
    private DispatcherTimer? _hideTimer;
    private int _showGeneration;
    private TimeSpan _currentVisibleFor;
    private bool _isFadingOut;
    private bool _isFadingIn;
    // Separate from _showGeneration on purpose: _showGeneration ticks on every ShowOsd call,
    // including the ones that deliberately leave a running fade-in alone, so using it to
    // decide whether a fade-in is still current would strand _isFadingIn at true forever.
    private int _fadeInGeneration;
    private PresentationMode _activeMode;

    // Set by OnForegroundCoversMonitorChanged before anything else runs, so every path that
    // reads it — including ApplyPresentationMode's own poller-start guard below — sees the
    // current value rather than a stale one from before this event.
    private bool _coversMonitor;

    // Local mirror of ThemeService.BuildAccentOverride(). Lives on this ContentControl's
    // own Resources.MergedDictionaries so DynamicResource lookups inside OsdContent hit
    // it before walking up. See the comment on ThemeService.BuildAccentOverride for why
    // we can't rely on Application.Resources changes reaching an HwndSource RootVisual.
    private ResourceDictionary? _accentOverride;

    // Edit-mode state. Only touched from the UI dispatcher.
    private bool _isEditMode;
    private SettingsModel? _preEditSnapshot;
    private readonly List<PositionOverlayWindow> _overlays = new();

    /// <summary>Binding root for the OSD content. Exposes CardHost's visible-card list;
    /// the shell itself holds no per-card state.</summary>
    public OsdShellViewModel Shell { get; }

    /// <summary>Raised when the OSD enters or exits position-edit mode so Settings
    /// can flip its Edit/Save/Cancel button state.</summary>
    public event Action<bool>? EditModeChanged;

    public OsdHost(SettingsService settings, ThemeService theme, CardHost cardHost, NotchHomeState home)
    {
        _settings = settings;
        _theme = theme;
        _cardHost = cardHost;
        _home = home;
        Shell = new OsdShellViewModel(cardHost);
        _presentation = new ClassicPresentation(this);
        _hoverPoller = new NotchHoverPoller(Dispatcher, _log);
        _hoverPoller.HoverChanged += OnNotchHoverChanged;
        _hoverPoller.DraggingOverChanged += OnDragApproachChanged;

        _hoverPoller.Polled += ResyncClickThrough;
        Application.Current.Exit += (_, _) => _hoverPoller.Dispose();

        ZBandID = NativeMethods.GetTopMostZBandID();
        // Recorded once at startup because it silently decides whether the OSD can cover an
        // exclusive-fullscreen game, and nothing else in the log reveals it. See UiAccess.
        _log?.Info("OsdHost", Plith.Interop.UiAccess.Describe());
        TopMost = true;
        Activatable = false;      // never steal focus
        IsClickThrough = false;   // mouse hover keep-alive needs hit-testing
        Opacity = 0;
        Focusable = false;

        _content = new OsdContent { DataContext = Shell, Log = _log };
        ApplyShellWidth();
        Content = _content;

        _widgets.PageRequested += OnWidgetPageRequested;
        _widgets.SetPages(_pager, BuildWidgetPages());

        // Re-measure the notch's surface whenever the content's laid-out size actually changes.
        //
        // Reposition() measures once, synchronously, and that single reading is not enough. A
        // hover adds the ambient card to CardHost's collection, and the ItemsControl bound to it
        // materialises the new container on a LATER layout pass — so the Measure/UpdateLayout
        // pair running immediately afterwards can still see the old tree. The window then grows
        // to the real height once layout settles, while NotchSurface keeps the stale one.
        //
        // On a running build that drew the ambient row on the panel and left the media and volume
        // cards hanging below it with no surface underneath at all, rendered straight over
        // whatever was on screen. SizeChanged fires after arrange, so it reports the size the
        // cards are actually drawn at rather than a prediction of it.
        _content.SizeChanged += (_, e) =>
        {
            if (_presentation is not AmbientNotchPresentation) return;
            if (e.NewSize.Width <= 0 || e.NewSize.Height <= 0) return;
            _presentation.OnContentMeasured(e.NewSize);

            // And re-anchor, because in notch mode the width is no longer fixed.
            //
            // Reposition centres the window by subtracting its width from the screen's. That was
            // safe while the width was a constant; it stopped being safe the moment the panel
            // was allowed to size to its content, because the window is placed and THEN grows -
            // so the centring used whatever width it had a frame earlier, and the notch drifted
            // off to the right. Re-anchoring on the measurement that caused the growth is what
            // keeps the two in step.
            Reposition();
        };

        // Seed the local accent mirror BEFORE the HwndSource is created (in CreateWindow
        // below) so the very first paint already uses the picked accent. Subsequent
        // updates come from ThemeService.ThemeApplied.
        RefreshAccentMirror();
        _theme.ThemeApplied += OnThemeApplied;

        // PreviewMouseLeftButtonDown rather than a button: the pill is drawn by the notch's
        // surface, not by an interactive element, and preview means the click opens the panel
        // before anything inside it can swallow the event.
        PreviewMouseLeftButtonDown += (_, _) => OnNotchClicked();
        HorizontalWheel += OnHorizontalWheel;


        MouseEnter += OnMouseEnter;
        MouseLeave += OnMouseLeave;

        // The IsClickThrough setter no-ops until the window is loaded (see BandWindow.Ext),
        // and Loaded is raised asynchronously — after App.OnStartup's synchronous constructor
        // chain has already run ApplyPresentationMode below. So the value that call writes
        // reaches the dependency property but never reaches the HWND's WS_EX_TRANSPARENT bit,
        // which was fixed at CreateWindow() time from the IsClickThrough = false above.
        // Without this re-assert the notch launches hit-testable and swallows clicks at the
        // top of the screen — window drag-to-maximise, browser tabs, Snap Layouts — until the
        // first full show/hide cycle happens to resync it. Derived from WantsHitTesting rather
        // than asserted, like every other IsClickThrough write in this class, so it can never
        // disagree with the presentation about what "hit-testable" means. _presentation is
        // assigned at the top of this constructor, so it is non-null however early this fires.
        Loaded += (_, _) => IsClickThrough = !_presentation.WantsHitTesting;

        // A settings save can change where the OSD sits (position / monitor), so re-anchor it
        // without waiting for the next pop. Card-level settings (colour thresholds) are owned by
        // AudioCard and MediaCard and never travel through the shell — compact mode is the
        // exception, because it changes the CARD'S WIDTH, which is shell geometry.
        _settings.Changed += _ => Dispatcher.BeginInvoke(() =>
        {
            ApplyShellWidth();

            // Only when the answer changed. SetPages resets the pager and rebuilds the dots, and
            // doing that on every save would snap an open notch back to its first page whenever
            // anything at all was tweaked.
            if (_settings.Current.ShowWeather != _weatherPageInstalled) ApplyWidgetPages();

            if (_settings.Current.Presentation != _activeMode)
            {
                _activeMode = _settings.Current.Presentation;
                ApplyPresentationMode();
                return;   // ApplyPresentationMode repositions as part of settling the mode
            }
            Reposition();
        });

        // Pre-create the native HWND so the first ShowOsd is instant.
        // BandWindow.CreateWindow is idempotent if HasSourceCreated is already true.
        CreateWindow();

        _activeMode = _settings.Current.Presentation;
        ApplyPresentationMode();
    }

    // While a window covers the monitor, the notch does not merely shrink away — it stops
    // being a notch. The spec's phrase for the covered state is "behave like Classic", and the
    // first real session showed how much that phrase was carrying: retraction hid the shape but
    // Reposition() still pinned the OSD to top-centre, so a volume key in a game put the card in
    // the dead centre of the field of view. That is the single most intrusive spot on the screen,
    // and the user's own Classic anchor sat near the bottom, chosen deliberately.
    //
    // Building Classic here makes the fallback literal rather than partial: the anchor, the edge
    // margin, the card's shape and the transition all come from Classic, because the object
    // driving them IS Classic. It also means there is exactly one place that decides what the
    // OSD currently is, instead of a covered-state special case in each of them.
    //
    // Safe to rebuild on this edge only because FullscreenVideoWatcher now applies hysteresis to
    // the falling edge: the raw signal flapped several times a second during gameplay, which
    // would have thrashed presentations here.
    // The setting alone. A covering window used to force Classic here, which meant a fullscreen
    // video or a maximised browser silently changed the OSD's shape AND its position — the card
    // appeared at the user's saved Classic spot, typically nowhere near the top of the screen.
    // What a game needs is the resting strip gone, not the notch gone; AmbientNotchPresentation
    // hides its window at rest while covered, which is the same thing Classic did.
    private bool WantsNotch =>
        _settings.Current.Presentation == PresentationMode.AmbientNotch
        && !(_coversMonitor && _settings.Current.UseClassicOverFullscreen);

    private IOsdPresentation BuildPresentation() => WantsNotch
        ? new AmbientNotchPresentation(this, _content, () => _settings.Current.NotchStripHeightDip,
                                       () => _coversMonitor, _log)
        : new ClassicPresentation(this);

    // Switching modes rebuilds the presentation and returns the window to that mode's rest
    // state. Both directions need cleaning up after the other: Classic leaves Opacity at 0,
    // the notch leaves an expansion value and a visible surface.
    private void ApplyPresentationMode()
    {
        _hideTimer?.Stop();
        _isFadingIn = false;
        _isFadingOut = false;

        BeginAnimation(OpacityProperty, null);
        _content.BeginAnimation(OsdContent.NotchExpandProperty, null);
        _content.NotchExpand = 0;

        _presentation = BuildPresentation();
        IsClickThrough = !_presentation.WantsHitTesting;

        // Before Reposition(), deliberately: SetNotchLook drops the content root's top margin,
        // which changes the measured content height, and that measurement is what the notch
        // opens into. Reshaping after measuring would size the open panel from the other mode's
        // geometry and leave it a margin short.
        _content.SetNotchLook(_presentation is AmbientNotchPresentation);

        // After SetNotchLook and before Reposition: the width is what Reposition measures
        // against, and a mode switch changes it. Still derived from the PRESENTATION rather than
        // the setting — the two now agree in notch mode, but the presentation remains the thing
        // actually being drawn, and deriving from what is drawn is the rule this branch keeps
        // relearning.
        ApplyShellWidth();

        if (_presentation is AmbientNotchPresentation notch)
        {
            // A mode switch while the panel is open must settle fully closed, the same as
            // reaching rest normally does — otherwise the ambient row would survive a
            // rebuild that just re-parked the strip underneath it.
            _home.Close();
            Opacity = Math.Clamp(_settings.Current.OsdOpacityPercent, 50, 100) / 100.0;
            Reposition();               // measures content, which calls OnContentMeasured

            // PrepareShow only when the strip is going to be on screen. Park() decides between a
            // visible pill and a hidden window by itself, so calling Show() first and letting
            // Park() immediately hide it again would be a Show/Hide pair around a covering
            // window - and Show on a layered window is not guaranteed to be free of a render.
            if (!_coversMonitor) notch.PrepareShow();
            notch.Park();               // settle straight into rest; no descent flash
        }
        else
        {
            // Classic has no home view. This covers both the settings switch and the
            // covered-monitor fallback, because both rebuild through here.
            _home.Close();
            Opacity = 0;
            Hide();
        }

        // Started/stopped here rather than in the branches above so it happens after
        // Reposition() has already published a fresh HoverRect for the notch case, and after
        // _presentation has already been reassigned in both cases. The latter matters: Stop()
        // can raise a synthetic HoverChanged(false) if the poller thought the cursor was still
        // inside the strip when the mode switched away from notch, and by the time that fires
        // here, _presentation is already the new ClassicPresentation — so OnNotchHoverChanged's
        // own "not AmbientNotchPresentation" guard discards it instead of restarting a hide
        // timer or toggling IsClickThrough for a mode switch that has already settled its own
        // state.
        //
        // The _coversMonitor check IS needed now. It used not to be, because BuildPresentation
        // returned Classic while covered, so a notch presentation existing at all proved nothing
        // was covering the monitor. The notch now survives a covering window with its window
        // hidden — and polling for a hover over a strip that is not on screen would wake the
        // notch up over a game, which is the one thing none of this may do.
        if (_presentation is AmbientNotchPresentation && !_coversMonitor) _hoverPoller.Start();
        else _hoverPoller.Stop();

        // Recorded because the mode a run is actually in cannot be recovered any other way,
        // and almost every manual check in docs/PHASE6-VERIFICATION.md is conditional on it.
        // Without this line "the strip never appeared" and "the mode was never applied" look
        // identical from a log, which is the same ambiguity UiAccess.Describe() exists to
        // remove — and the same one that cost real time to rediscover in Phase 5.
        // IsClickThrough is included because the notch parks click-through, and whether that
        // actually took effect is the single highest-risk unverified behaviour on this branch:
        // BandWindow's setter silently no-ops until the window is loaded.
        _log?.Info("OsdHost",
            $"Presentation applied: {_settings.Current.Presentation}" +
            $", clickThrough={IsClickThrough}" +
            $", coversMonitor={_coversMonitor}" +
            (_presentation is AmbientNotchPresentation n
                ? $", parked={n.IsAtRest(1.0)}, stripHeight={_settings.Current.NotchStripHeightDip:0.#}"
                : string.Empty));
    }

    /// <summary>
    /// Called when a window starts or stops covering the monitor. Classic ignores it entirely.
    ///
    /// For the notch this is a mode change, not a visibility tweak. The spec's phrase for the
    /// covered state is "behave like Classic", and the first real session showed that hiding
    /// the strip alone does not deliver it: Reposition() still pinned the OSD to top-centre, so
    /// a volume key inside a game put the card in the dead centre of the field of view, while
    /// the user's own Classic anchor sat near the bottom of the screen where they had put it.
    ///
    /// So the covered state builds ClassicPresentation outright (see BuildPresentation), and
    /// this handler just re-runs the mode switch. That teardown already does everything the
    /// covered state needs and used to be duplicated here: it stops the hide timer, clears both
    /// the opacity and content-offset animations, clears _isFadingIn and _isFadingOut — which
    /// matters because removing a clock does not raise its Completed handler, so the flags would
    /// otherwise strand — reshapes the card, repositions, and starts or stops the hover poller.
    ///
    /// Rebuilding on this edge is only affordable because FullscreenVideoWatcher applies
    /// hysteresis to the falling edge. The raw signal flapped several times a second during
    /// gameplay; against that, this would have thrashed presentation objects continuously.
    /// </summary>
    public void OnForegroundCoversMonitorChanged(bool covers)
    {
        if (_coversMonitor == covers) return;
        _coversMonitor = covers;

        // Classic is already Classic, but it still has somewhere to go: a resting Classic OSD is
        // a window DWM is still compositing over whatever is covering the monitor, and hiding
        // outright is the only state that costs a game nothing.
        if (_settings.Current.Presentation != PresentationMode.AmbientNotch)
        {
            if (covers) _presentation.HideWindowIfPossible();
            return;
        }

        // Still a full rebuild rather than a flag flip. The mode no longer changes, but the
        // resting state does, and settling into a rest is what this does.
        ApplyPresentationMode();

        // ApplyPresentationMode settles the new rest state but does not always take the window
        // down: Park() hides it when covered, and a path that ends anywhere else would leave a
        // window composited over the game. Belt and braces, on the side that costs frames.
        if (covers) _presentation.HideWindowIfPossible();
    }

    private void OnThemeApplied()
    {
        // Marshal to the UI thread in case ThemeApplied ever fires from a different
        // context (Windows preference watcher already dispatches, but defend anyway).
        if (!Dispatcher.CheckAccess())
        {
            Dispatcher.BeginInvoke(new Action(RefreshAccentMirror));
            return;
        }
        RefreshAccentMirror();
    }

    private void RefreshAccentMirror()
    {
        var dict = _theme.BuildAccentOverride();
        if (_accentOverride is not null)
            Resources.MergedDictionaries.Remove(_accentOverride);
        Resources.MergedDictionaries.Add(dict);
        _accentOverride = dict;
        // The tinted surface changes the card padding illusion but doesn't move geometry;
        // still, force a re-measure so a redraw is queued right away rather than waiting
        // for the next natural render pass. Cheap — no layout invalidation upstream.
        _content?.InvalidateVisual();
    }

    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool SetWindowPos(nint hWnd, nint hWndInsertAfter, int X, int Y, int cx, int cy, uint uFlags);

    private static readonly nint HWND_TOPMOST = -1;
    private const uint SWP_NOSIZE = 0x0001;
    private const uint SWP_NOMOVE = 0x0002;
    private const uint SWP_NOACTIVATE = 0x0010;
    private const uint SWP_SHOWWINDOW = 0x0040;
    private const uint SWP_HIDEWINDOW = 0x0080;

    private Plith.Services.Shelf.DropChannelServer? _dropChannel;
    private Plith.Services.Shelf.ShelfSession? _shelfSession;

    /// <summary>
    /// Why the notch is down, or None when it is not.
    ///
    /// It was a bool, and a bool cannot answer the question EndStandAside has to ask. The two
    /// reasons have opposite lifetimes: a drag's stand-aside is armed to withdraw after 450 ms,
    /// because nothing distinguishes a file being carried to the top of the screen from a window
    /// being dragged there, while the shelf was asked for out loud and stays until it is
    /// dismissed. The drag detector keeps running under an open shelf and will raise a
    /// departure, so without the reason the shelf's own notch would come back up underneath it.
    /// </summary>
    private enum StandAsideReason { None, Drag, Shelf }

    private StandAsideReason _standAside;

    /// <summary>
    /// Hand the drop catcher over. Set by App once the channel exists, which is after this
    /// window is constructed — hence a property rather than a constructor parameter.
    /// </summary>
    public void AttachDropChannel(Plith.Services.Shelf.DropChannelServer channel) => _dropChannel = channel;

    /// <summary>
    /// Hand over the conversation the shelf runs on, and take its three answers.
    ///
    /// Opened and Closed drive the notch; Unavailable is the one that reaches the person, so it
    /// goes to the page they just clicked rather than to the log alone.
    /// </summary>
    public void AttachShelfSession(Plith.Services.Shelf.ShelfSession session)
    {
        _shelfSession = session;
        session.Opened += OnShelfOpened;
        session.Closed += OnShelfClosed;   // carries WHY, which decides whether the notch parks
        session.Unavailable += OnShelfUnavailable;
        // Paging while the catcher holds the frame. It arrives here because the pager is here.
        session.PageRequested += OnShelfPageRequested;
        session.Shown += OnShelfShown;
    }

    /// <summary>
    /// A drag has arrived at the notch, or has left it.
    ///
    /// Plith cannot receive the drop and never will: UIAccess puts it at High integrity and UIPI
    /// refuses Explorer's cross-integrity call. So it steps out of the way instead — the window
    /// goes down and the catcher, a Medium process, takes the same rectangle for as long as the
    /// drag lasts.
    ///
    /// The window has to go down rather than merely yield z-order. The catcher cannot enter the
    /// UIAccess band, so while the notch is up the catcher is underneath it and the drop lands
    /// on a window that cannot take it — which is exactly the state this whole design exists to
    /// leave behind.
    /// </summary>
    private void OnDragApproachChanged(bool approaching)
    {
        if (_presentation is not AmbientNotchPresentation)
        {
            // Classic has no shelf. Nothing to stand aside for, and standing aside would hide an
            // OSD the person may be reading.
            return;
        }

        if (approaching) BeginStandAside();
        else EndStandAside();
    }

    private void BeginStandAside()
    {
        if (_standAside != StandAsideReason.None) return;

        if (_dropChannel is not { IsConnected: true })
        {
            // No catcher, so hiding would buy nothing and cost the notch. Logged rather than
            // silent: this is what a catcher that failed to start looks like from Plith's side,
            // and it is otherwise indistinguishable from no drag having happened.
            _log?.Info("Shelf", "Drag arrived but no catcher is connected; staying put.");
            return;
        }

        var target = NotchGeometry.DropTargetRect(_hoverPoller.HoverRect);
        var (x, y, w, h) = NotchGeometry.DipToPhysical(target, _hoverPoller.DpiScale);

        _standAside = StandAsideReason.Drag;
        HideForCatcher();
        _ = _dropChannel.SendAsync(new Plith.Services.Shelf.DropMessage(
            Plith.Services.Shelf.DropVerb.Show, x, y, w, h, []));

        _log?.Info("Shelf", $"Standing aside for a drag: {x},{y} {w}x{h}.");
    }

    /// <summary>
    /// A drop landed and the store kept it. Go STRAIGHT from the drop pill to the filled shelf.
    ///
    /// THREE WINDOWS BECAME ONE HANDOVER, and the three were what a person reported as the moment
    /// after a drop not being smooth. The old path was: the catcher's stand-in pill goes away,
    /// Plith's notch comes back at its resting strip, and then the shelf is asked for and appears.
    /// Three shapes at the same place inside a second, with Plith's own notch flickering between
    /// two of the catcher's windows.
    ///
    /// Here the notch never comes back. The stand-aside simply changes REASON, from a drag to the
    /// shelf, so Plith's window stays down throughout, and the order on the wire does the rest:
    /// OpenShelf first, the stand-in's Hide after it. The channel serialises sends in call order
    /// (see DropChannelServer.SendAsync), so the shelf's window is up before the pill goes, and
    /// the swap happens under a surface that is already painted.
    ///
    /// The store is written BEFORE this is called, which is the other half of "filled": Open
    /// sends the item list as it is at that moment, so a shelf opened before the Add would arrive
    /// empty and gain the file a beat later, which is the same flicker in a different place.
    /// </summary>
    public void OnDropLanded()
    {
        if (!Dispatcher.CheckAccess())
        {
            Dispatcher.BeginInvoke(new Action(OnDropLanded));
            return;
        }

        // Not a notch presentation, no shelf page, or no session: fall back to the old path,
        // which puts the notch back. Better a flicker than a notch that never returns.
        if (_shelfSession is null || _shelfPageIndex < 0 || _presentation is not AmbientNotchPresentation)
        {
            OnCatcherStoodDown();
            return;
        }

        ApplyWidgetPages();

        // The frame is the shelf's page, and the pager has to agree: the catcher draws the rail
        // from the index this sends, and a rail pointing at another page would be the one piece
        // of chrome that disagrees with what is on screen.
        var before = _pager.Index;
        if (_pager.GoTo(_shelfPageIndex)) _widgets.SyncToPager(Math.Sign(_pager.Index - before));
        _content.SetPanelContent(NotchPanelContent.Widgets);

        // Taken out of the DRAG's stand-aside by hand rather than through EndStandAside, which
        // would restore the notch on its way past. The reason changes to Shelf below, when the
        // session reports itself open.
        _standAside = StandAsideReason.None;

        _shelfSession.RailPageCount = _pager.PageCount;
        _shelfSession.RailShelfIndex = _shelfPageIndex;
        _shelfSession.Open(_hoverPoller.HoverRect, _hoverPoller.DpiScale);

        // The pill, AFTER the shelf. If the session refused (no catcher connected, which it
        // reports through Unavailable), the notch has to come back instead, and the guard below
        // is how this tells the two apart: OnShelfOpened sets the reason to Shelf.
        if (_standAside == StandAsideReason.Shelf)
        {
            _ = _dropChannel?.SendAsync(new Plith.Services.Shelf.DropMessage(
                Plith.Services.Shelf.DropVerb.Hide, 0, 0, 0, 0, []));
            _log?.Info("Shelf", "Drop landed; straight into the shelf, no notch in between.");
        }
        else
        {
            _log?.Info("Shelf", "Drop landed but the shelf did not open; putting the notch back.");
            _ = _dropChannel?.SendAsync(new Plith.Services.Shelf.DropMessage(
                Plith.Services.Shelf.DropVerb.Hide, 0, 0, 0, 0, []));
            RestoreNotch();
        }
    }

    /// <summary>
    /// Stand aside for the shelf rather than for a drop.
    ///
    /// The same mechanism as BeginStandAside and deliberately not the same method: that one
    /// arms a withdrawal after 450 ms, because nothing distinguishes a file being carried to the
    /// top of the screen from a window being dragged there. The shelf was asked for, so it
    /// stays until it is dismissed.
    ///
    /// The rectangle is read BEFORE anything is hidden, and the ORDER of the two steps after it
    /// is the other half of the design. The notch does not go down here; it goes down in
    /// OnShelfOpened, which the session raises only once it has decided the catcher is there to
    /// send to. Hiding first and restoring on failure would mean a click on a broken install
    /// blinks the notch out and back, and the sentence explaining why would arrive into a page
    /// that had just been hidden.
    /// </summary>
    public void OpenShelf()
    {
        if (!Dispatcher.CheckAccess())
        {
            Dispatcher.BeginInvoke(new Action(OpenShelf));
            return;
        }

        if (_shelfSession is null) return;
        if (_presentation is not AmbientNotchPresentation) return;
        if (_standAside != StandAsideReason.None) return;

        _shelfSession.Open(_hoverPoller.HoverRect, _hoverPoller.DpiScale);
    }

    /// <summary>
    /// Who holds the frame: Plith, or the catcher.
    ///
    /// THE ONE PLACE THAT DECIDES, called after every page commit, after the frame opens, and
    /// when it collapses. The shelf is a page in the notch's frame now, and the catcher is what
    /// draws that page, for a reason that is not a preference: a file can only be dragged out of
    /// the catcher's window (Plith is high integrity in Release and DoDragDrop carries nothing
    /// from there), and a press cannot be delegated between processes. Both measured, in
    /// docs/SHELF-VERIFICATION.md section 4. So landing on the shelf page has to BE the handover;
    /// anything later is a press that has already gone to the wrong window.
    ///
    /// Idempotent, because three callers ask and the answer is usually "no change". The session
    /// itself refuses a second Open while the shelf is up, but relying on that would make this
    /// method's correctness depend on another class's guard.
    /// </summary>
    private void ReconcileShelfFrame()
    {
        if (!Dispatcher.CheckAccess())
        {
            Dispatcher.BeginInvoke(new Action(ReconcileShelfFrame));
            return;
        }

        if (_shelfSession is null || _shelfPageIndex < 0) return;

        // Any handover this call was waiting to make is cancelled here, unconditionally, and
        // restarted below only if it is still the right thing to do. Every path that changes
        // whether the catcher should hold the frame comes through this method, so cancelling at
        // the top is what keeps a deferred handover from acting on a decision that has since
        // changed: a drag starting, the page turning past the shelf, the notch closing.
        _handoverWait?.Stop();

        // A DRAG IN FLIGHT WINS, always, and this guard is the difference between a file landing
        // and a file going nowhere. While a drag is near the notch the catcher's window is its
        // STAND-IN, which is the thing that accepts the drop; the shelf page taking that window
        // instead would put a surface with no drop handling under the pointer at the exact moment
        // a file is being released onto it. Measured from a real session's log: a held cursor
        // entered the band and no "Standing aside for a drag" followed it.
        //
        // Refused rather than deferred. When the drag ends the notch comes back on whatever page
        // it was on, and if that is the shelf the next gesture hands it over as usual.
        if (_standAside == StandAsideReason.Drag) return;

        var wantsCatcher = _presentation is AmbientNotchPresentation notch
                        && notch.IsOpenEnoughToShowContent
                        && _content.PanelContent == NotchPanelContent.Widgets
                        && _pager.Index == _shelfPageIndex;

        if (wantsCatcher == (_standAside == StandAsideReason.Shelf)) return;

        // THE GESTURE ENDS AT THE HANDOVER, in both directions, and without this the swipe that
        // arrives at the shelf carries straight past it.
        //
        // Measured from a real session's log, reported as the notch closing by itself the moment
        // the shelf appeared:
        //
        //     59.029  Widget page committed: delta=8, index=3/4   <- the shelf page, reached
        //     59.045  Shelf requested ... / Standing aside
        //     59.322  Widget page committed: delta=2, index=0/4   <- 275 ms later, past it
        //     59.323  Shelf closed by Plith: the page turned away from it
        //
        // A precision touchpad sends deltas of two and eight, and the pager accumulates them to
        // its 120 threshold, so one continuous swipe legitimately pages more than once. That is
        // the right rule between Plith's own pages, where the surface never changes. It is the
        // wrong rule here: the handover swaps the window between two processes, takes about 300
        // ms, and during that swap the accumulated intent belongs to a surface that has already
        // gone. Resting the accumulator means a second page costs a second gesture.
        _pager.Rest();

        if (wantsCatcher)
        {
            // NOT WHILE A PAGE IS STILL MOVING.
            //
            // The handover replaces this window with the catcher's, so it ends every animation
            // running in this one. MEASURED from the logs: the page turn commits, the catcher's
            // window is opaque 151 ms later and this window goes down, and the outgoing page's
            // 260 ms slide is cut off at 58 per cent of its travel. Reported as the transition to
            // the shelf not being smooth like the other four, which it was not: the other four
            // finish their movement.
            //
            // So the gesture animates to completion in ONE process, and the frame changes hands
            // after it. The catcher's own arrival fade then runs on a page that has stopped
            // moving, which is the thing section 10.26 found could not be done while it was still
            // going.
            var moving = _widgets.SlideRemainingMs;
            if (moving > 0)
            {
                _handoverWait ??= new DispatcherTimer(DispatcherPriority.Send, Dispatcher);
                _handoverWait.Interval = TimeSpan.FromMilliseconds(moving);
                _handoverWait.Tick -= OnHandoverWaitElapsed;
                _handoverWait.Tick += OnHandoverWaitElapsed;
                _handoverWait.Start();
                return;
            }

            _shelfHandoverAt = Environment.TickCount64;

            // The rail's shape travels with the open: only this class knows how many pages there
            // are, and without it the catcher would draw no rail at all and the chrome would
            // blink out on one page in five.
            _shelfSession.RailPageCount = _pager.PageCount;
            _shelfSession.RailShelfIndex = _shelfPageIndex;
            OpenShelf();
        }
        else
        {
            _shelfHandoverAt = Environment.TickCount64;
            _shelfSession.Close();
        }
    }

    /// <summary>The page turn has finished moving, so the frame may change hands now. Decided
    /// again from scratch rather than acted on: 260 ms is long enough for the page, the drag
    /// state or the notch itself to have changed.</summary>
    private void OnHandoverWaitElapsed(object? sender, EventArgs e)
    {
        _handoverWait?.Stop();
        ReconcileShelfFrame();
    }

    /// <summary>Waits out a page turn before handing the frame to the catcher. See the deferral
    /// in <see cref="ReconcileShelfFrame"/>.</summary>
    private DispatcherTimer? _handoverWait;

    /// <summary>
    /// How long after a handover a forwarded wheel delta is treated as the tail of the gesture
    /// that caused it rather than as a new one.
    ///
    /// 400 ms. The swap itself takes about 25 ms and the measured tail arrived at 234, so this
    /// covers it with room; a person who means to page again has to pause for less than half a
    /// second, which is shorter than the time it takes to see that the shelf has arrived.
    /// </summary>
    private const long ShelfSettleMs = 400;

    /// <summary>When the frame last changed hands, for <see cref="ShelfSettleMs"/>.</summary>
    private long _shelfHandoverAt;

    // SlideDirectionTo is DELETED, and the history is the point.
    //
    // It returned 0 for the shelf's page so that page turn alone did not slide, because the
    // catcher's window used to arrive part-way through the movement and a slide interrupted at
    // 58 per cent reads as a stutter. Sliding it in the CATCHER instead was tried first and was
    // worse: two processes animating one page turn across a window handover cannot line up (see
    // ShelfWindow, where SlidePageIn used to be, and section 10.26).
    //
    // Both of those were treating the symptom. The cause was that the handover happened WHILE the
    // page was moving, and the fix is in ReconcileShelfFrame: the gesture now animates to
    // completion in this process, and the frame changes hands after it has stopped. With nothing
    // left to interrupt, the shelf's page slides exactly like the other four, which is what a
    // person asked for twice.


    /// <summary>
    /// A paging gesture arrived from the catcher while it was holding the frame.
    ///
    /// Fed to the SAME two methods a gesture on Plith's own window reaches, rather than to the
    /// pager directly: those methods hold the guards (edit mode, the presentation, whether the
    /// frame is open enough to page at all) and a second path around them would page a notch that
    /// is not there. A delta of zero means the index is meant, and vice versa.
    /// </summary>
    private void OnShelfPageRequested(int delta, int index)
    {
        if (!Dispatcher.CheckAccess())
        {
            Dispatcher.BeginInvoke(new Action(() => OnShelfPageRequested(delta, index)));
            return;
        }

        // THE TAIL OF THE GESTURE THAT BROUGHT US HERE IS NOT A NEW GESTURE.
        //
        // Reported as: I move toward the shelf and the moment I arrive the notch closes. From the
        // person's own log, with a precision touchpad:
        //
        //     22.647  Widget page committed: delta=-6, index=3/4   <- the shelf, reached
        //     22.664  Shelf requested / Standing aside
        //     22.881  Widget page committed: delta=-3, index=2/4   <- 234 ms later, back off it
        //     22.881  Shelf closed by Plith: the page turned away from it
        //
        // A touchpad sends a STREAM of small deltas (six, three) rather than discrete notches of
        // 120, so one flick easily carries enough total to page twice. Between Plith's own pages
        // that is correct and wanted. Across the handover it is not: the window has just been
        // swapped between two processes, and the deltas arriving in the next fraction of a second
        // were part of the flick that asked for THIS page.
        //
        // Resting the accumulator at the handover (see ReconcileShelfFrame) was not enough, and
        // the log above is why: the stream continues, and 234 ms of it is another 120.
        //
        // A RAIL CLICK IS NEVER IGNORED. It carries an index rather than a delta, and a click is
        // a decision rather than momentum.
        if (delta != 0 && Environment.TickCount64 - _shelfHandoverAt < ShelfSettleMs)
        {
            _log?.Info("Shelf", $"Ignored a forwarded wheel delta of {delta}: it arrived " +
                                $"{Environment.TickCount64 - _shelfHandoverAt} ms after the handover.");
            return;
        }

        if (delta != 0) OnHorizontalWheel(this, delta);
        else OnWidgetPageRequested(this, index);
    }

    private void OnShelfOpened()
    {
        if (!Dispatcher.CheckAccess())
        {
            Dispatcher.BeginInvoke(new Action(OnShelfOpened));
            return;
        }

        _standAside = StandAsideReason.Shelf;

        // THE WINDOW DOES NOT GO DOWN HERE ANY MORE, and that is the difference between a
        // handover and a blink.
        //
        // Opened fires when the REQUEST goes out. The catcher's window arrives about 25 ms later,
        // and hiding on this event left those milliseconds with nothing on screen, which a person
        // reported as the shelf closing and instantly reopening. The catcher answers with
        // ShelfShown once its window is up (see OnShelfShown), and the hide happens there.
        //
        // With a TIMEOUT, because an answer that never comes must not leave Plith's window over
        // the shelf: in a Release build Plith is in the UIAccess band and the catcher is not, so
        // Plith's window sits ABOVE it, and a missing hide would mean a shelf nobody can see.
        // The fallback is the old behaviour, which was merely ugly.
        _shelfShownWait?.Stop();
        _shelfShownWait ??= new DispatcherTimer(DispatcherPriority.Send, Dispatcher);
        _shelfShownWait.Interval = ShelfShownTimeout;
        _shelfShownWait.Tick -= OnShelfShownTimedOut;
        _shelfShownWait.Tick += OnShelfShownTimedOut;
        _shelfShownWait.Start();

        // The hide timer is deliberately LEFT RUNNING, and stopping it was the first version.
        //
        // Stopping it means the notch is frozen in whatever shape it had when it went down, and
        // the shelf closing would then put an open widget frame back on screen with nothing left
        // to collapse it: the notch would stay open until the next event, which is a panel across
        // the top of the screen that nothing takes away.
        //
        // Left running, both outcomes are right. With hover keep-alive on, the pointer is inside
        // the panel rectangle for as long as the shelf covers it, so the timer keeps re-arming
        // and the notch comes back open and collapses once the pointer leaves: the same rule a
        // panel under the pointer already follows. With it off, the notch collapses while it is
        // hidden and comes back at rest. The collapse animates on a window nobody can see, which
        // costs one storyboard and no correctness: its completion only re-parks the presentation
        // and re-derives click-through, and its HideWindowIfPossible is guarded by the same
        // covered-monitor condition RestoreNotch is.
        _log?.Info("Shelf", "Standing aside for the shelf.");
    }

    /// <summary>How long Plith waits for the catcher to say its window is up before hiding
    /// anyway. Generous next to the 25 ms the answer actually takes, and short enough that a
    /// catcher which never answers costs a flicker rather than a visible wait.</summary>
    private static readonly TimeSpan ShelfShownTimeout = TimeSpan.FromMilliseconds(400);

    private DispatcherTimer? _shelfShownWait;

    /// <summary>
    /// The catcher's shelf is on screen. Take Plith's window down, with nothing visible between
    /// the two.
    /// </summary>
    private void OnShelfShown()
    {
        if (!Dispatcher.CheckAccess())
        {
            Dispatcher.BeginInvoke(new Action(OnShelfShown));
            return;
        }

        _shelfShownWait?.Stop();

        // Guarded, because this arrives from another process and may arrive late: a ShelfShown
        // for a shelf that has already closed must not hide the notch that came back.
        if (_standAside != StandAsideReason.Shelf) return;

        HideForCatcher();
    }

    private void OnShelfShownTimedOut(object? sender, EventArgs e)
    {
        _shelfShownWait?.Stop();
        if (_standAside != StandAsideReason.Shelf) return;

        _log?.Warn("Shelf", "The catcher never said its window was up; hiding anyway after " +
                            $"{ShelfShownTimeout.TotalMilliseconds:0} ms.");
        HideForCatcher();
    }

    private void OnShelfClosed(ShelfCloseCause cause)
    {
        if (!Dispatcher.CheckAccess())
        {
            Dispatcher.BeginInvoke(new Action(() => OnShelfClosed(cause)));
            return;
        }

        if (_standAside != StandAsideReason.Shelf) return;

        _standAside = StandAsideReason.None;

        // THE FRAME GOES DOWN BEFORE THE WINDOW COMES BACK, and without this a person sees the
        // shelf twice in a row, in two different designs.
        //
        // RestoreNotch only shows the window; it changes nothing about what is inside it. What is
        // inside it is the widget frame, still open, still on the shelf page, from the moment
        // ReconcileShelfFrame handed the frame over. So the catcher's shelf disappears and
        // Plith's own imitation of the same page takes its place for whatever the hide timer has
        // left, then collapses. Measured from a real session's log: "Shelf closed; notch back" at
        // 44.011 and the ambient card leaving at 44.839, so 800 ms of the wrong shelf. Reported
        // as a clock appearing, then a shelf, then the notch closing.
        //
        // There is nothing to come back TO. The shelf went away because the pointer left it or
        // because the person cleared it, and either way they are done with it; the notch belongs
        // at rest.
        //
        // Park() rather than FadeOutAndHide(), which is the animated path the hide timer takes:
        // the window is still hidden at this point, so there is no collapse for anyone to watch,
        // and animating one only delays the window coming back by its duration. The three things
        // that path's completion does and that matter here are done below.
        // ONLY WHEN THE SURFACE ENDED, never when the person paged past the shelf. The two
        // causes want opposite things and treating them alike is what a person reported as "when
        // I try to scroll to the other widgets the notch closes":
        //
        //     25.045  Widget page committed: delta=120, index=0/4
        //     25.045  Shelf closed by Plith: the page turned away from it
        //     25.050  Shelf closed; notch back      <- and the notch was parked here
        //
        // The shelf is the last page, so one notch past it wraps to the clock. Plith closes the
        // shelf, which is right, and then parked the whole notch, which threw away the page the
        // person had just asked for. Paging through the shelf now leaves the frame open on
        // whatever page they landed on: SyncToPager has already moved it.
        if (cause == ShelfCloseCause.Surface && _presentation is AmbientNotchPresentation notch)
        {
            _hideTimer?.Stop();
            notch.Park();

            // The panel is closed, so the notch is click-through again. ShowOsd's forward half
            // only ever turns this OFF; every path back to rest owes the backward half.
            IsClickThrough = !_presentation.WantsHitTesting;

            // And the ambient row closes with the panel it lived in, exactly as it does at the
            // end of the ordinary fade-out.
            _home.Close();
        }

        RestoreNotch();

        // No Hide sent back. That verb belongs to the catcher's OTHER window, the one that
        // stands in during a drag; the shelf has already taken itself down, which is how this
        // event got here.
        _log?.Info("Shelf", "Shelf closed; notch back.");
    }

    /// <summary>
    /// The shelf could not be shown, with the reason as a sentence.
    ///
    /// It goes onto the shelf page, because that is the thing the person just clicked. A click
    /// that does nothing is indistinguishable from the product being broken, and this is the one
    /// interaction in the product whose failure mode is entirely invisible: the helper process
    /// is not something anyone knows exists.
    /// </summary>
    private void OnShelfUnavailable(string why)
    {
        if (!Dispatcher.CheckAccess())
        {
            Dispatcher.BeginInvoke(new Action(() => OnShelfUnavailable(why)));
            return;
        }

        _shelfPage?.ShowUnavailable(why);
        _log?.Warn("Shelf", $"Shelf unavailable: {why}");
    }

    /// <summary>
    /// Take the notch off screen for the catcher, WITHOUT moving or resizing it.
    ///
    /// SWP_NOMOVE and SWP_NOSIZE are load-bearing, not decoration. The four zeros are the
    /// conventional filler for "I am not touching position or size", but that meaning lives in
    /// the flags; without them the zeros are a real instruction and the window goes to 0,0.
    /// Measured at the physical console on 2026-09-19: the shelf opened centred at 1088,0 and
    /// the notch came back at 0,0, in the top-left corner, 1088 px from where it belongs, and
    /// stayed there until the next Reposition happened to run. scripts/check-win32-flags.ps1
    /// fails the build if either flag is dropped again.
    /// </summary>
    private void HideForCatcher()
    {
        if (Handle == 0) return;

        _ = SetWindowPos(Handle, HWND_TOPMOST, 0, 0, 0, 0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_HIDEWINDOW);

        // Logged because the GAP between this moment and the catcher's window being opaque is
        // the whole handover, and it has been reported as the notch closing and reopening twice.
        // Read it against the catcher's own "Shelf opened" line: this should land AFTER it by
        // about the length of the arrival fade, never before it.
        _log?.Info("Shelf", "Window down: the catcher's shelf says it is opaque.");
    }

    /// <summary>
    /// Only back up if the notch has somewhere to be. While a window covers the monitor the
    /// resting state IS hidden, and re-showing here would put a permanently composited strip
    /// back over a game, the one thing the covered state exists to prevent.
    ///
    /// SWP_NOMOVE and SWP_NOSIZE for the same reason as HideForCatcher above: this call shows a
    /// window, it does not place one. Without them it showed the notch in the top-left corner.
    /// </summary>
    private void RestoreNotch()
    {
        if (Handle != 0 && !_coversMonitor)
            _ = SetWindowPos(Handle, HWND_TOPMOST, 0, 0, 0, 0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW);
    }

    /// <summary>
    /// The catcher is no longer standing in — it took a drop, or it withdrew because no file
    /// drag ever materialised. Either way the notch comes back.
    ///
    /// Needed as a second route because the poller cannot supply one: the detector is still in
    /// its approaching state while the button is held, so it raises no transition, and the notch
    /// would stay down until the person let go of a window they were dragging somewhere else.
    /// </summary>
    public void OnCatcherStoodDown()
    {
        if (!Dispatcher.CheckAccess())
        {
            Dispatcher.BeginInvoke(new Action(OnCatcherStoodDown));
            return;
        }

        EndStandAside();
    }

    /// <summary>
    /// The DRAG's stand-aside ends. Called from the approach detector and from the catcher
    /// standing down, and it must leave an open shelf alone.
    ///
    /// The detector keeps polling while the shelf is up, and it will raise a departure the
    /// moment the pointer leaves the band, which is most of the time, since the shelf is taller
    /// than the band that opened it. Without this guard the notch would come straight back up
    /// underneath a shelf the person is still using, and the shelf's own close would then find
    /// nothing to restore.
    /// </summary>
    private void EndStandAside()
    {
        if (_standAside != StandAsideReason.Drag) return;
        _standAside = StandAsideReason.None;

        _ = _dropChannel?.SendAsync(new Plith.Services.Shelf.DropMessage(
            Plith.Services.Shelf.DropVerb.Hide, 0, 0, 0, 0, []));

        RestoreNotch();

        _log?.Info("Shelf", "Drag over; notch back.");
    }

    /// <summary>Re-assert HWND_TOPMOST so a game / video player that raised itself topmost
    /// after our last ShowOsd doesn't sit above us. Safe to call repeatedly: SetWindowPos
    /// with NOACTIVATE leaves the foreground window's focus untouched.</summary>
    public void ReassertTopmost()
    {
        if (Handle == 0) return;
        _ = SetWindowPos(Handle, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
    }

    private void OnMouseEnter(object sender, System.Windows.Input.MouseEventArgs e)
    {
        if (_isEditMode) return;

        // Never let a hover OPEN the notch — it may only keep an already-open panel alive.
        //
        // This guard exists because making the notch hit-testable at every expansion (so a
        // per-point WM_NCHITTEST filter could run) also turned WPF's own mouse events back on
        // for it. This handler then did what it was written to do for Classic and called
        // SnapToVisible, which sets NotchExpand straight to 1. The result on a running build:
        // the notch opened fully the instant the pointer touched it, click-to-open never got a
        // chance, and because that path starts no hide timer it never closed again.
        if (_presentation is AmbientNotchPresentation notch && !notch.IsOpenEnoughToShowContent)
            return;
        if (!_settings.Current.HoverKeepAlive) return;
        // CardHost is the single authority for when the OSD appears. Whether a
        // faded-out (Opacity 0) layered window still hit-tests mouse messages has never
        // been verified — if it does, hovering over the OSD's screen region while
        // fullscreen-video suppression is active would resurrect it here, bypassing the
        // suppressor entirely. Guard defensively rather than find out live.
        if (_cardHost.Suppressor?.IsSuppressed == true) return;
        _hideTimer?.Stop();
        // SnapToVisible below removes the in-flight fade-out (Classic) or collapse (notch) clock,
        // and WPF raises no Completed for a clock removed that way — so FadeOutAndHide's
        // completion never runs and _isFadingOut would stick true forever. Both OnMouseLeave and
        // OnNotchHoverChanged's exit branch early-return on that flag, so no hide timer would
        // ever be restarted: in Classic this stranded the OSD at full opacity until the next
        // volume key (a defect that predates the notch), and in notch mode it leaves a fully
        // open panel sitting on screen indefinitely instead of shrinking back to the resting shape.
        // Cleared here rather than inside SnapToVisible because the flag is
        // OsdHost's transition bookkeeping, not the presentation's.
        _isFadingOut = false;
        // A show transition in flight is already on its way to fully visible. Snapping here
        // would clear its animation mid-expansion (SnapToVisible's BeginAnimation(..., null))
        // and turn the notch's expansion into a jump — this is how the notch becoming
        // hit-testable partway through a hover-triggered expansion used to cancel its own
        // animation the instant WPF delivered the resulting MouseEnter.
        if (_isFadingIn) return;
        _presentation.SnapToVisible(Math.Clamp(_settings.Current.OsdOpacityPercent, 50, 100) / 100.0);
    }

    // Entering the resting notch opens it and makes the panel interactive; leaving
    // hands back to the ordinary hide timer. IsClickThrough is toggled here rather than
    // inside the presentation because it is a window-level concern and OsdHost owns the
    // window — but it is deliberately re-DERIVED from _presentation.WantsHitTesting rather
    // than asserted as an independent true/false, so this handler can never disagree with
    // WantsHitTesting about what "hit-testable" means. Forcing click-through back on the
    // instant the cursor leaves the resting rectangle (before the collapse even starts) would
    // be wrong: the panel is still fully open at that point, so WantsHitTesting is still
    // true and a media transport button underneath it must still receive the click. The sync
    // below reflects that: on entry it goes false immediately, because ShowOsd's
    // AnimateToVisible/SnapToVisible flips the presentation's parked flag synchronously,
    // before any animation runs; on exit it stays false until AnimateToRest actually
    // completes and re-parks, which FadeOutAndHide and ShowOsd both re-sync (see the
    // comments at those two call sites for why neither alone is enough).
    // Note this is the first code in Plith to change IsClickThrough after the HWND exists —
    // see the manual check in docs/PHASE6-VERIFICATION.md.
    // Hover acknowledges the pointer; a CLICK opens the panel. See AmbientNotchPresentation.Peek
    // for why: an open panel is solid to the mouse across its whole width, so opening on hover put
    // the OSD over browser tabs and window controls and took the clicks meant for them whenever
    // the pointer passed near the top of the screen. Reported on a running build exactly that way.
    private void OnNotchHoverChanged(bool inside)
    {
        if (_isEditMode) return;
        if (_cardHost.Suppressor?.IsSuppressed == true) return;
        if (_presentation is not AmbientNotchPresentation notch) return;

        if (inside)
        {
            notch.Peek();
        }
        else
        {
            // Only withdraw a peek. If the user clicked and opened it, the pointer leaving the
            // pill must not shut it — the hide timer owns that, and its keep-alive check asks the
            // poller whether the cursor is still inside the open panel.
            if (!notch.IsOpenEnoughToShowContent) notch.Unpeek();
        }

        IsClickThrough = !_presentation.WantsHitTesting;
    }

    /// <summary>
    /// A sideways wheel gesture on the notch. Only pages a notch that is already open enough to
    /// be showing content: a swipe over the resting pill is not a request to page through
    /// something that is not on screen, and a swipe during the peek is part of reaching for it.
    ///
    /// The open/closed question is asked of the presentation, which reads it from the live
    /// expansion value, never from a flag. Every stale-state defect on this branch came from
    /// the other choice.
    /// </summary>
    /// <summary>
    /// The widget pages, in paging order.
    ///
    /// One page today. The frame handles that honestly - it draws no dots, because a single dot
    /// says nothing except that there is nowhere to go - and the pager refuses to page at all,
    /// so a swipe over a one-page frame does nothing rather than appearing to break.
    /// </summary>
    private static List<FrameworkElement> BuildWidgetPages() => [new Widgets.ClockWidget()];

    /// <summary>
    /// Give the widget frame an audio page.
    ///
    /// Called by App once the orchestrator exists, rather than injected through the constructor:
    /// the orchestrator needs this window's Dispatcher to be built at all, so at construction
    /// time there is nothing to hand over. Rebuilding the pages is the frame's normal path -
    /// pages come and go anyway - and the pager is told the new count as part of it.
    /// </summary>
    /// <summary>Set alongside the audio source so the HUD's speaker can mute. Held rather than
    /// passed straight through because the HUD is built here, after the pages.</summary>
    private Func<bool>? _toggleMute;

    public void AttachAudioSource(AudioCardViewModel audio, Func<double, bool> write, MediaViewModel media,
                                  Func<WeatherSnapshot?> weather, Func<bool>? toggleMute = null,
                                  Action? openSource = null, Func<MicrophoneSnapshot?>? microphone = null,
                                  Func<bool?>? toggleMicMute = null, BrightnessCardViewModel? brightness = null)
    {
        _toggleMute = toggleMute;
        // Built once and kept. The widgets own timers and storyboards, so rebuilding them on
        // every settings change would churn exactly the resources this slice bounds — only the
        // LIST is rebuilt, and only when the weather page's presence actually has to change.
        // No audio page. A volume key already stretches the notch into a HUD that shows the
        // level the instant it changes, so a widget page showing the same number is a second
        // place for one fact - and the one you reach by swiping, long after the moment it
        // mattered. The draggable track it carried moves to the HUD's speaker instead.
        _media = media;
        _clockPage = new Widgets.ClockWidget(media, weather, microphone);
        // The typed city, read through a delegate so the page follows a settings change without
        // being rebuilt. Empty when the location is resolved from Windows or an IP lookup, and
        // the page hides the line rather than naming a place it cannot name.
        _weatherPage = new Widgets.WeatherWidget(weather, ReadRevealDate, WriteRevealDate, _log,
                                                 () => _settings.Current.WeatherLocation);
        _mediaPage = new Widgets.MediaWidget(media, openSource);

        BuildShelfPage();
        ApplyWidgetPages();

        _hud = new Widgets.NotchHud(audio, media, _toggleMute, brightness);
        _content.SetWidgetContent(_widgets, _hud);
    }

    /// <summary>
    /// Tell the weather page a new reading arrived.
    ///
    /// Called from App, which owns the WeatherService. The alternative — handing the widget the
    /// service — would put a view in front of a background timer and an HttpClient for the sake
    /// of one event.
    /// </summary>
    public void OnWeatherUpdated() => _weatherPage?.OnWeatherUpdated();

    /// <summary>The microphone's mute changed. The now page reads the state where it draws it,
    /// so it only needs telling that something moved.</summary>
    public void OnMicrophoneChanged() => _clockPage?.Refresh();

    private Widgets.ShelfWidget? _shelfPage;
    private int _shelfPageIndex = -1;

    /// <summary>Where the media page sits in the current list, so the opening rule can name it.
    /// Read from the same list that installs the pages, so the two cannot disagree about an order
    /// they both take from one place.</summary>
    private int _mediaPageIndex = -1;

    /// <summary>The media view model, held so the opening-page rule can ask what is playing.
    /// Null until AttachAudioSource runs, which is why NotchOpeningPolicy takes a bool rather
    /// than the view model.</summary>
    private ViewModels.MediaViewModel? _media;

    /// <summary>
    /// Open the notch on the shelf page, because a file just landed there.
    ///
    /// Without this the whole gesture ends in silence: the notch steps aside, the catcher takes
    /// the drop, the notch comes back, and nothing anywhere says the file arrived. Reported from
    /// a live run as the feature having crashed — which is the right reading of an interaction
    /// that gives no answer.
    ///
    /// The frame rather than a HUD, and that is the exception to this window's own rule. An event
    /// normally gets the HUD because an answer to something you did must not look like a place
    /// you went; here the answer IS a place — the shelf now holds something, and the page showing
    /// what it holds is the acknowledgement.
    /// </summary>
    public void ShowShelfLanding()
    {
        if (!Dispatcher.CheckAccess())
        {
            Dispatcher.BeginInvoke(new Action(ShowShelfLanding));
            return;
        }

        if (_presentation is not AmbientNotchPresentation) return;

        ApplyWidgetPages();
        if (_shelfPageIndex < 0) return;

        var before = _pager.Index;
        if (_pager.GoTo(_shelfPageIndex)) _widgets.SyncToPager(Math.Sign(_pager.Index - before));

        _content.SetPanelContent(NotchPanelContent.Widgets);

        // AND THE HANDOVER, so what a person sees after a drop is the shelf rather than Plith's
        // imitation of it.
        //
        // Reported from a real session as something weird appearing for a couple of seconds after
        // a drag, and it was two designs one second apart: this acknowledgement drew Plith's own
        // ShelfWidget (small tiles, its own hint line) while the shelf itself, one hover later,
        // is the catcher's page (tiles with a ground, the name in the chrome row, real previews).
        // The same shelf, twice, looking different.
        //
        // It costs a cross-process swap for a 2.6 second glance, which is the same swap every
        // page turn onto the shelf already pays. Paying it here buys the thing this whole slice
        // is about: there is ONE shelf.
        ReconcileShelfFrame();

        // fromHover: true is what stops ShowOsd taking the frame straight back off us and
        // replacing it with a HUD. The flag names the click path rather than a hover, and this
        // is the same kind of caller — something that has already decided which panel it wants.
        ShowOsd(TimeSpan.FromMilliseconds(2600), fromHover: true);
    }
    private Plith.Services.Shelf.ShelfStore? _shelf;
    private bool _shelfPageInstalled;
    private bool _pagesInstalled;

    /// <summary>
    /// Hand the shelf over. Set by App alongside the drop channel, and it may arrive before or
    /// after the pages are built — whichever happens second installs the page.
    /// </summary>
    public void AttachShelf(Plith.Services.Shelf.ShelfStore shelf)
    {
        _shelf = shelf;

        // The page's presence follows the shelf's contents, so a shelf that fills up while the
        // notch is idle has to be able to install it from here.
        _shelf.Changed += () =>
        {
            if (!Dispatcher.CheckAccess()) { Dispatcher.BeginInvoke(new Action(OnShelfStoreChanged)); return; }
            OnShelfStoreChanged();
        };

        if (_clockPage is not null) BuildShelfPage();
    }

    /// <summary>
    /// The store changed. Installs or removes the shelf page if its presence changed, and says
    /// what the store now holds.
    ///
    /// The log line is an instrument rather than diagnostics for their own sake: the notch's own
    /// shelf page repaints from this same event, so a report of tiles flickering back onto a
    /// surface is only answerable if the count the repaint was given is on the record next to the
    /// count Plith sent the catcher. Both are now written, from the two places that know them.
    /// </summary>
    private void OnShelfStoreChanged()
    {
        _log?.Info("Shelf", $"Store changed: {_shelf?.Items.Count ?? -1} item(s).");
        ApplyWidgetPages();
    }

    private void BuildShelfPage()
    {
        if (_shelf is null || _shelfPage is not null) return;

        // No click to subscribe to any more. The page had one while it was the only way in, and
        // the handover happens on the page turn now: landing on the shelf page IS the shelf. A
        // click as a second route would be a second way to start a cross-process window swap,
        // which is the kind of thing that ends up racing itself.
        _shelfPage = new Widgets.ShelfWidget(_shelf);
        ApplyWidgetPages();
    }

    private Widgets.ClockWidget? _clockPage;
    private Widgets.WeatherWidget? _weatherPage;
    private Widgets.MediaWidget? _mediaPage;

    /// <summary>Whether the weather page is currently in the pager, so a settings change that
    /// does not affect it does not rebuild the list.</summary>
    private bool _weatherPageInstalled;


    /// <summary>
    /// Put the right pages in the frame for the current settings.
    ///
    /// "Show weather" off removes the page, which is what Settings says it does. Leaving it in
    /// and letting it render "Weather unavailable" would make the setting a lie in the one
    /// direction a person can check — and this is the second time on this branch a settings
    /// string described something the code did not do.
    /// </summary>
    private void ApplyWidgetPages()
    {
        if (_clockPage is null || _weatherPage is null || _mediaPage is null) return;

        var wantsWeather = _settings.Current.ShowWeather;

        // Always present, which is the opposite of the weather page's rule and deliberately so.
        // It was conditional first, on the reasoning that an empty page exists to say nothing is
        // there — and that reasoning is wrong for this page, because a shelf nobody can see is a
        // feature nobody discovers. Weather absent means there is no reading; a shelf absent
        // means the person never learns they can drop a file on the notch at all. The empty page
        // carries that sentence.
        var wantsShelf = _shelfPage is not null;

        // Nothing to do when neither page's presence changed. The shelf raises Changed on every
        // drop, and rebuilding the list each time would discard and re-add live pages that own
        // timers and storyboards — for a list that came out identical.
        if (_pagesInstalled && wantsWeather == _weatherPageInstalled && wantsShelf == _shelfPageInstalled)
            return;

        List<FrameworkElement> pages = [_clockPage];
        if (wantsWeather) pages.Add(_weatherPage);
        pages.Add(_mediaPage);
        _mediaPageIndex = pages.Count - 1;
        if (wantsShelf) pages.Add(_shelfPage!);

        _shelfPageIndex = wantsShelf ? pages.Count - 1 : -1;

        _widgets.SetPages(_pager, pages);
        _weatherPageInstalled = wantsWeather;
        _shelfPageInstalled = wantsShelf;
        _pagesInstalled = true;
    }

    /// <summary>
    /// Which answer the HUD should give.
    ///
    /// The reason travels with the request rather than being remembered on the way past. It
    /// used to be dropped at CardHost's event boundary, which would have left the shell guessing
    /// the shape from whichever cards happened to be visible - inferring something CardHost
    /// already knew, and getting it wrong whenever both a track and a level had changed.
    ///
    /// A null reason means the caller is not an event at all: a hover, a mode switch, edit mode.
    /// Those never reach the HUD branch, and the volume default is only what an unexpected
    /// caller would get.
    /// </summary>
    /// <summary>
    /// Size the shell for whatever is actually being shown.
    ///
    /// Asks the live presentation, not the setting. They disagree exactly when it matters: while
    /// a window covers the monitor the notch falls back to Classic, and a width taken from the
    /// setting would leave the card laid out inside a shell sized for a notch.
    /// </summary>
    private void ApplyShellWidth() =>
        _content.SetShellWidth(_presentation is AmbientNotchPresentation, _settings.Current.CompactMode);

    /// <summary>
    /// The last day the weather page played its arrival, read from settings.
    ///
    /// Parsed rather than trusted: this is a hand-editable ini file, and an unparseable value
    /// means "never", which costs one extra reveal. Throwing here would take down a background
    /// widget for the sake of a decoration.
    /// </summary>
    private DateOnly? ReadRevealDate() =>
        DateOnly.TryParseExact(_settings.Current.WeatherRevealDate, "yyyy-MM-dd",
            CultureInfo.InvariantCulture, DateTimeStyles.None, out var date)
            ? date
            : null;

    private void WriteRevealDate(DateOnly date)
    {
        _settings.Current.WeatherRevealDate = date.ToString("yyyy-MM-dd", CultureInfo.InvariantCulture);
        _settings.Save(_settings.Current);
    }

    private static NotchHudKind PickHudKind(ShowReason? reason) => reason switch
    {
        ShowReason.MediaChange or ShowReason.MediaCommand => NotchHudKind.Media,
        // Named rather than left to the default. Brightness used to fall through to Volume, so
        // a brightness key drew the speaker and the volume level: the right answer existed in
        // the card stack, which an event never reaches in notch mode.
        ShowReason.BrightnessChange => NotchHudKind.Brightness,
        _ => NotchHudKind.Volume,
    };

    private void OnWidgetPageRequested(object? sender, int index)
    {
        var before = _pager.Index;
        if (!_pager.GoTo(index)) return;
        _widgets.SyncToPager(Math.Sign(_pager.Index - before));
        ReconcileShelfFrame();
    }

    private void OnHorizontalWheel(object? sender, int delta)
    {
        if (_isEditMode) return;
        if (_presentation is not AmbientNotchPresentation notch) return;
        if (!notch.IsOpenEnoughToShowContent) return;

        // Environment.TickCount64 rather than DateTime: monotonic, and the pager only ever
        // compares two of them. Time is passed in rather than read inside the pager so the
        // gap rule stays testable.
        if (!_pager.Accumulate(delta, Environment.TickCount64)) return;

        _widgets.SyncToPager(Math.Sign(delta));

        // Logged because this is the only way the user's own touchpad can be characterised
        // later: the commit threshold and the rearm floor are provisional constants, and
        // whether they are right can only be read back from a real gesture. Instrument from
        // inside; three external sampling harnesses during slice 2 all gave misleading answers.
        _log?.Info("OsdHost", $"Widget page committed: delta={delta}, index={_pager.Index}/{_pager.PageCount}");

        // The page turn IS the handover, in both directions.
        ReconcileShelfFrame();
    }

    /// <summary>
    /// A click on the drawn notch opens it.
    ///
    /// Clicks anywhere else never arrive here, and not because anything filters them: the window
    /// is layered with per-pixel transparency, so the system hit-tests the ALPHA WPF rendered.
    /// Transparent pixels pass the mouse through with no code involved, which also means the hit
    /// region follows whatever shape is currently drawn — the resting pill, a HUD, the open
    /// frame — without anything having to keep a rectangle in step with it.
    /// </summary>
    private void OnNotchClicked()
    {
        if (_isEditMode) return;
        if (!_settings.Current.ShowNotchWidgets) return;
        if (_cardHost.Suppressor?.IsSuppressed == true) return;
        if (_presentation is not AmbientNotchPresentation notch) return;

        // A HUD counts as closed for this purpose, and that is the point rather than a loophole.
        // The HUD is an answer to a volume key; clicking it is a person saying "and now show me
        // the rest", which is exactly what the widget frame is. Without this the notch was open
        // enough to swallow the click and not open enough to be worth having clicked.
        var showingHud = _content.PanelContent == NotchPanelContent.Hud;
        if (!showingHud && notch.IsOpenEnoughToShowContent) return;   // already the frame — let it have the click

        // Reset here, on the way in, rather than in the collapse's completion callback. The
        // spec defers remembering the last page across opens, so every open starts at the first
        // one either way - and a value written in a Completed handler is the exact hazard that
        // produced five defects on this branch. Recomputing it where it is used cannot go stale.
        // Computed here, on the way in, rather than remembered. See NotchOpeningPolicy: the page
        // is a function of what is playing right now, and the carousel spec's deferral of
        // remembering the last page is kept deliberately. The zero passed to SyncToPager is the
        // slide DIRECTION, not a page: an opening frame does not slide.
        _pager.ResetTo(NotchOpeningPolicy.OpeningPage(_media?.IsPlaying == true, _mediaPageIndex));
        _widgets.SyncToPager(0);

        // A click opens the widget frame. An event opens the card stack, and ShowOsd's other
        // callers switch it back - see SetWidgetMode's comment for why the two are exclusive.
        _content.SetPanelContent(NotchPanelContent.Widgets);

        _home.Open();
        _hideTimer?.Stop();

        // Deferred one dispatcher turn so the ambient row exists before the expansion animates.
        // Open() adds the card to CardHost's collection, but the bound ItemsControl gains its
        // container during WPF's DataBind pass rather than synchronously, so animating straight
        // away opened the panel in two visible stages — media and volume first, the clock and
        // weather arriving late. UpdateLayout() does not help: it runs measure and arrange
        // without flushing that queue.
        var visibleFor = TimeSpan.FromMilliseconds(_settings.Current.ShowDurationMs);
        Dispatcher.BeginInvoke(new Action(() =>
        {
            // A mode switch, edit mode or a covering window can all land between the click and
            // this callback, and each makes the show wrong rather than merely late.
            if (_isEditMode) return;
            if (_presentation is not AmbientNotchPresentation) return;
            if (!_home.IsOpen) return;

            ShowOsd(visibleFor, fromHover: true);
        }), DispatcherPriority.Loaded);
    }





    /// <summary>
    /// Re-derive the window's click-through bit from the presentation, every poll.
    ///
    /// Every other write to IsClickThrough hangs off a discrete event, and several of those ride
    /// an animation's Completed callback. WPF raises no Completed for a clock that a competing
    /// animation replaced — the hazard that has produced four defects on this branch — so a
    /// single dropped callback can leave a parked notch hit-testable. That is not a cosmetic
    /// failure: the window is invisible while parked, 440 DIP wide, and sits across the top of
    /// the screen where windows are dragged to maximise and browser tabs live, so it silently
    /// eats clicks that were never meant for it. Reported on a running build exactly that way.
    ///
    /// This recomputes the same expression the event handlers use rather than caching a second
    /// answer, so it can correct a missed write without ever disagreeing with them. The setter
    /// itself is a no-op when the value is unchanged, and the native toggle underneath returns
    /// early when the style bits already match, so the steady-state cost is a bool comparison.
    /// </summary>
    private void ResyncClickThrough()
    {
        if (_isEditMode) return;
        bool want = !_presentation.WantsHitTesting;
        if (IsClickThrough != want) IsClickThrough = want;
    }

    private void OnMouseLeave(object sender, System.Windows.Input.MouseEventArgs e)
    {
        if (_isEditMode) return;
        if (!_settings.Current.HoverKeepAlive) return;
        if (_currentVisibleFor <= TimeSpan.Zero) return;
        if (_isFadingOut) return;
        RestartHideTimer(_currentVisibleFor);
    }

    private void RestartHideTimer(TimeSpan visibleFor)
    {
        _hideTimer?.Stop();
        _hideTimer = new DispatcherTimer { Interval = visibleFor };
        _hideTimer.Tick += (_, _) =>
        {
            _hideTimer!.Stop();

            // Keep-alive for the notch, decided here rather than on a hover transition.
            //
            // WPF's IsMouseOver cannot answer this for the OSD, and that was measured on a
            // running build rather than assumed: the window is WS_EX_LAYERED with per-pixel
            // alpha, Windows hit-tests layered windows against that alpha, and at rest the notch
            // is a couple of opaque DIP in an otherwise transparent window. The cursor that
            // triggered the hover sits over transparent space; the panel opens beneath a now
            // stationary cursor; no further WM_MOUSEMOVE is generated, so Windows never
            // re-evaluates and MouseEnter never fires. IsMouseOver stayed false for the panel's
            // entire life while the user was looking straight at it.
            //
            // Asking the poller on a transition was tried first and was also wrong: moving up
            // toward the notch crosses the panel's rectangle BEFORE the resting shape's, so the
            // enter transition fires while the notch is still parked. Whatever that early
            // transition decides is final, because a cursor that then stays inside produces no
            // second transition.
            //
            // Checking at the moment of hiding has neither problem. It needs no transition, no
            // mouse movement and no message delivery — just where the pointer is, right now,
            // when it matters.
            if (_presentation is AmbientNotchPresentation
                && _settings.Current.HoverKeepAlive
                && _hoverPoller.IsCursorInPanel)
            {
                RestartHideTimer(visibleFor);
                return;
            }

            FadeOutAndHide();
        };
        _hideTimer.Start();
    }

    /// <param name="reason">Why the OSD is appearing. Only the notch uses it, and only to pick
    /// the HUD's shape; null means "not an event", which is what every internal caller is.</param>
    public void ShowOsd(TimeSpan visibleFor, bool fromHover = false, ShowReason? reason = null)
    {
        if (_isEditMode) return;   // edit mode keeps its own always-on visibility

        // Closed here, not only in FadeOutAndHide's AnimateToRest completion. IsAtRest treats
        // an in-flight collapse as "at rest" (AmbientNotchPresentation's _isCollapsing), so an
        // event arriving mid-collapse — a volume key, not a hover — lands in the
        // AnimateToVisible branch below, which calls BeginAnimation(NotchExpandProperty, expand)
        // and REPLACES the running collapse clock. WPF raises no Completed for a clock removed
        // that way (the identical hazard is already documented on _isFadingOut in OnMouseEnter
        // above), so the collapse's own completion — and the _home.Close() inside it — never
        // runs. Closing here instead makes the row's closure not depend on that clock finishing.
        // fromHover is the one exception: OnNotchHoverChanged's hover-in branch has just called
        // _home.Open() and legitimately wants the row to stay for this show.
        if (!fromHover) _home.Close();

        // Same reasoning, same place: an event is not a request to go anywhere, so it takes the
        // panel back from the widget frame. fromHover is the click path, which has already asked
        // for the frame and must not have it taken away one turn later.
        //
        // Which shape it takes back depends on the mode. Classic shows its card, unchanged since
        // 0.1.5. The notch shows a HUD - short, wide, nothing to press - because an answer to
        // something you did must not look like a place you went. Falling back to the cards when
        // the HUD has not been attached yet keeps a very early event (before App has wired the
        // view models) showing something rather than an empty panel.
        if (!fromHover)
        {
            // WHO CAUSED IT decides whether the open frame survives. The rule itself lives in
            // NotchEventPolicy, free of WPF, because this class is a BandWindow that the test
            // project cannot construct - and two earlier versions of this rule reached a running
            // build with no test between them. See that file for both, and for the log lines
            // from the run where the third case was measured doing harm.
            var frameIsOpen = _content.PanelContent == NotchPanelContent.Widgets
                              && _presentation is AmbientNotchPresentation open
                              && open.IsOpenEnoughToShowContent;

            // Whether the page being looked at already displays this event: a track change while
            // the media page is up is an answer you can already see, and replacing the frame with
            // a HUD would take away the place you deliberately went to in order to show you what
            // is already there.
            var pageAlreadyShowsIt = frameIsOpen
                                     && PickHudKind(reason) == NotchHudKind.Media
                                     && ReferenceEquals(_widgets.CurrentPage, _mediaPage);

            var keepFrame = NotchEventPolicy.KeepsOpenFrame(reason, frameIsOpen, pageAlreadyShowsIt);
            var wantsHud = !keepFrame && _presentation is AmbientNotchPresentation && _hud is not null;
            if (wantsHud) _hud!.Show(PickHudKind(reason));
            if (!keepFrame)
                _content.SetPanelContent(wantsHud ? NotchPanelContent.Hud : NotchPanelContent.Cards);
        }

        _showGeneration++;
        bool wasFadingOut = _isFadingOut;
        _isFadingOut = false;
        _currentVisibleFor = visibleFor;
        double targetOpacity = Math.Clamp(_settings.Current.OsdOpacityPercent, 50, 100) / 100.0;
        bool wasAtRest = _presentation.IsAtRest(targetOpacity);

        if (wasAtRest)
        {
            Reposition();
            _presentation.PrepareShow();

            // Only start a fade-in when one is not already running toward this same target.
            // Volume keys repeat far faster than a fade takes, so a held or spammed key lands
            // several events inside a single fade — and restarting the animation on each of
            // them made the OSD pulse instead of staying up. The other half of this fix — not
            // clearing the animation before restarting it — lives in
            // ClassicPresentation.AnimateToVisible, next to the animation it protects.
            if (wasFadingOut || !_isFadingIn)
            {
                // Logged on the transition only, not on every repeat, so a held volume key
                // produces one line per appearance. This is the line that separates "the OSD
                // was never asked to show" from "it was shown and something on top of it won":
                // over a game in true exclusive fullscreen the display is scanned out from the
                // game's own swapchain, so nothing composites over it however correctly the
                // OSD behaves, and without this line the two cases look identical from a log.
                _log?.Info("OsdHost",
                    $"Show: transition at {Left:0},{Top:0} for {visibleFor.TotalMilliseconds:0}ms" +
                    (wasFadingOut ? " (interrupting hide)" : string.Empty));

                _isFadingIn = true;
                int gen = ++_fadeInGeneration;
                _presentation.AnimateToVisible(targetOpacity, () =>
                {
                    if (_fadeInGeneration == gen) _isFadingIn = false;
                });
            }
        }
        else
        {
            _presentation.SnapToVisible(targetOpacity);
            _isFadingIn = false;
        }

        // Both branches above already flipped the presentation's parked state synchronously
        // (AnimateToVisible and SnapToVisible both clear it before this line runs), so
        // WantsHitTesting already reports the descended value here. This is the forward half
        // of the click-through sync: FadeOutAndHide's resync only fires on the way BACK to
        // rest, so without this line a volume-key show — one nobody hovered into — would
        // leave IsClickThrough at whatever the last re-park left it (true), making the
        // descended card click-through and its transport buttons dead.
        IsClickThrough = !_presentation.WantsHitTesting;

        ReassertTopmost();

        if (_settings.Current.HoverKeepAlive && IsMouseOver) return;
        RestartHideTimer(visibleFor);
    }

    /// <summary>Take the OSD down now, ignoring its hide timer. Used when the suppression gate
    /// closes while the card is already on screen.</summary>
    public void HideOsd()
    {
        if (_isEditMode) return;   // edit mode owns its own always-on visibility
        _hideTimer?.Stop();
        if (_presentation.IsFullyHidden) return;
        // Already on the way out — restarting the animation from the current opacity would
        // stretch the fade instead of shortening it.
        if (_isFadingOut) return;
        FadeOutAndHide();
    }

    private void FadeOutAndHide()
    {
        var gen = _showGeneration;
        _isFadingOut = true;
        _isFadingIn = false;
        _presentation.AnimateToRest(() =>
        {
            if (_showGeneration != gen) return;
            _isFadingOut = false;
            // The presentation has just re-parked (AnimateToRest's own completion runs before
            // this callback), so WantsHitTesting now reports the resting value. This is the
            // backward half of the click-through sync — it covers every path back to rest,
            // hover-triggered or not — and pairs with the forward half in ShowOsd, which
            // covers every path back down to descended. Neither one alone is enough: this
            // one only ever turns click-through ON, ShowOsd's only ever turns it OFF.
            IsClickThrough = !_presentation.WantsHitTesting;

            // Closed at rest, not on cursor exit. Leaving the resting rectangle is the
            // normal way to move ONTO the open panel — OnNotchHoverChanged's exit branch
            // already relies on that — so closing there would take the row away at the
            // exact moment the user reached for it.
            _home.Close();

            // And then off the screen entirely, where the presentation allows it. Opacity 0 is
            // invisible to a person and not to DWM: the window stays composited, and a topmost
            // UIAccess-band window overlapping a full-screen game costs that game independent
            // flip. Measured by the user as 700 fps becoming 80.
            _presentation.HideWindowIfPossible();
        });
    }

    private void Reposition()
    {
        var m = _settings.Current;
        var screen = ResolveTargetScreen(m, notchActive: _presentation is AmbientNotchPresentation);
        if (screen is null) return;
        var area = screen.WorkingArea;

        _content.Measure(new Size(double.PositiveInfinity, double.PositiveInfinity));
        _content.UpdateLayout();
        var w = _content.DesiredSize.Width;
        var h = _content.DesiredSize.Height;
        if (w == 0 || h == 0) return;

        _presentation.OnContentMeasured(new Size(w, h));

        // The notch is defined by sitting at the top edge; Position is a Classic-only choice.
        // Settings disables the position editor in notch mode, but the stored Position value
        // survives a mode switch untouched (deliberately, so switching back to Classic restores
        // the user's anchor exactly), which means it is still BottomCenter here for most users.
        // Reads the ACTIVE presentation rather than the setting: while a covering window is up
        // the notch falls back to Classic wholesale, and that includes anchoring where the user
        // put their OSD instead of the top of the screen.
        var anchor = _presentation is AmbientNotchPresentation ? OsdPosition.TopCenter : m.Position;

        // Recorded because where the notch lands cannot be recovered any other way, and the one
        // report of it opening in the wrong place could equally be a stale width, the wrong
        // monitor, or a working area that is not the one being looked at. This line separates
        // those three without another round trip.
        _log?.Info("OsdHost",
            $"Reposition: anchor={anchor}, content={w:0}x{h:0}, " +
            $"screen='{screen.DeviceName}' area={area.Left:0},{area.Top:0} {area.Width:0}x{area.Height:0}");

        (Left, Top) = anchor switch
        {
            OsdPosition.BottomCenter => (area.Left + (area.Width - w) / 2, area.Bottom - h - _presentation.EdgeMarginDip),
            OsdPosition.BottomRight  => (area.Right - w - _presentation.EdgeMarginDip,   area.Bottom - h - _presentation.EdgeMarginDip),
            OsdPosition.TopCenter    => (area.Left + (area.Width - w) / 2, area.Top + _presentation.EdgeMarginDip),
            OsdPosition.TopRight     => (area.Right - w - _presentation.EdgeMarginDip,   area.Top + _presentation.EdgeMarginDip),
            OsdPosition.Custom       => CustomAnchor(area, w, h, m.CustomPositionXPercent, m.CustomPositionYPercent),
            _                        => (area.Left + (area.Width - w) / 2, area.Bottom - h - _presentation.EdgeMarginDip),
        };

        // Publish the collapsed pill's screen rectangle and the display's DPI scale to the
        // poller so it can compare against a fresh GetCursorPos reading. Both are computed
        // here, right after Left/Top settle, rather than inside the poller itself, which has no
        // route to either value on its own — NotchGeometry.PhysicalToDip is the only place
        // physical pixels and DIP meet; the rect and DpiScale below are DIP inputs to it, not
        // converted values themselves.
        //
        // w is passed as the window width so the pill can be centred in it. The pill's own
        // width is a constant and deliberately does NOT follow w: the hover target must stay
        // put when a media card widens the panel the pill opens into.
        if (_presentation is AmbientNotchPresentation)
        {
            _hoverPoller.HoverRect = NotchGeometry.HoverRect(
                Left, Top, w, _settings.Current.NotchStripHeightDip);
            // The whole window, which is the open panel's extent. Keep-alive is decided against
            // this rather than against WPF's IsMouseOver — see NotchHoverPoller.PanelHoverChanged
            // for why the input system cannot answer this question for a layered window.
            _hoverPoller.PanelRect = new Rect(Left, Top, w, h);
            _hoverPoller.DpiScale = VisualTreeHelper.GetDpi(_content).DpiScaleX;
        }
    }

    // Custom is stored as 0..1 fractions of the monitor working area, marking the
    // CENTRE of the OSD (not its top-left corner). This keeps the OSD visually
    // anchored when its content grows or shrinks (media card appearing / going away):
    // the centre stays fixed and the edges expand/contract symmetrically, so a card
    // dragged to the right side never overflows the screen when a longer media title
    // makes it wider. Clamped to a legal top-left range after applying the centre so
    // the OSD stays on-screen if the resolution shrinks or the content maxes out.
    private static (double left, double top) CustomAnchor(Rect area, double w, double h, double px, double py)
    {
        px = Math.Clamp(px, 0.0, 1.0);
        py = Math.Clamp(py, 0.0, 1.0);
        var centerX = area.Left + area.Width * px;
        var centerY = area.Top + area.Height * py;
        var left = Math.Clamp(centerX - w / 2, area.Left, Math.Max(area.Left, area.Right - w));
        var top = Math.Clamp(centerY - h / 2, area.Top, Math.Max(area.Top, area.Bottom - h));
        return (left, top);
    }

    // Choose the monitor a Custom-positioned OSD anchors on. Match by device name so a
    // resolution or scaling change on the same physical display keeps the OSD there.
    // Fall back to primary when the saved monitor is unplugged (external display gone).
    //
    // notchActive is whether the ACTIVE presentation is the notch, not whether the notch is
    // the configured mode. While a window covers the monitor the notch
    // falls back to ClassicPresentation wholesale, and the monitor is part of that fallback:
    // keying on the setting would anchor a BottomCenter OSD on the notch's saved display
    // rather than on the primary screen Classic would have used, contradicting the claim that
    // anchor, margin, shape and transition all come from Classic. One input, taken from the
    // object that is actually driving the window, rather than instance state read implicitly.
    private static Screen? ResolveTargetScreen(SettingsModel m, bool notchActive)
    {
        // The saved device name is honoured for Custom placement and for a live notch. Both
        // are "the user chose a display"; only the built-in anchors are display-agnostic.
        // ROADMAP §10 asked which monitor the notch pins to — this is the answer: the same
        // saved device name, matched the same way, falling back to primary when that
        // display is unplugged.
        bool usesSavedMonitor = m.Position == OsdPosition.Custom || notchActive;

        if (usesSavedMonitor && !string.IsNullOrEmpty(m.CustomPositionMonitorDeviceName))
        {
            foreach (var s in Screen.AllScreens)
            {
                if (string.Equals(s.DeviceName, m.CustomPositionMonitorDeviceName, StringComparison.OrdinalIgnoreCase))
                    return s;
            }
        }
        return Screen.PrimaryScreen;
    }

    // ============ Position edit mode ============

    /// <summary>True while the user is placing the OSD by dragging. Kept as a property
    /// so Settings can bind Save/Cancel button visibility to it.</summary>
    public bool IsInEditMode => _isEditMode;

    /// <summary>Enter overlay-driven position mode: dim every monitor with a
    /// PositionOverlayWindow (grid hotspots, Save/Cancel toolbar, and full drag
    /// detection over the OSD rectangle). Overlay tells the OSD where to move by
    /// firing PositionRequested; OsdHost applies snap/clamp and pushes the new
    /// rectangle back to every overlay so drag hit-testing stays accurate.</summary>
    public void EnterPositionEditMode()
    {
        if (_isEditMode) return;
        _isEditMode = true;
        _preEditSnapshot = _settings.Current.Clone();

        _hideTimer?.Stop();
        _isFadingOut = false;

        // Full opacity so the OSD is unmistakably visible above the dim layer.
        _presentation.SnapToVisible(Math.Clamp(_settings.Current.OsdOpacityPercent, 50, 100) / 100.0);
        Reposition();
        Show();
        ReassertTopmost();

        // Drag handling lives ENTIRELY inside PositionOverlayWindow: the overlay
        // owns the click surface (we know it does because hotspots work) and simply
        // checks whether the mouse-down landed inside the OSD's current rectangle.
        // That means the OSD itself does not need to receive clicks - keep it in
        // its normal click-through state so we do not fight the DWM/LAYERED hit-
        // test rules the OSD was originally built for.
        _log?.Info("OsdHost", "EnterPositionEditMode - drag lives on overlay canvas");

        // One overlay per monitor - hotspots and toolbar. Note: in UIAccess=false
        // builds the OSD is a regular Topmost window (not a band window), so the
        // last-shown Topmost wins the z-race. Overlays would end up ABOVE the OSD,
        // eating every click before it can reach the drag handlers - the reason
        // drag would appear dead while hotspot clicks still worked. Reassert OSD
        // topmost AFTER the overlays are created so the click surface goes back on
        // top of the dim layer.
        foreach (var screen in Screen.AllScreens)
        {
            var overlay = new PositionOverlayWindow(screen);
            overlay.PositionRequested += OnOverlayClickRequested;
            overlay.SaveRequested += OnOverlaySaveRequested;
            overlay.CancelRequested += OnOverlayCancelRequested;
            overlay.Show();
            _overlays.Add(overlay);
        }
        ReassertTopmost();
        BroadcastOsdRect();
        _log?.Info("OsdHost", $"overlays shown ({_overlays.Count}); OSD topmost reasserted; rect published");

        // Focus lands on the primary overlay so Esc / Enter shortcuts fire immediately.
        var primary = Screen.PrimaryScreen?.WorkingArea.Left ?? 0;
        var focus = _overlays.Find(o => Math.Abs(o.Left - primary) < 0.5) ?? _overlays[0];
        focus.Activate();
        _ = focus.Focus();

        EditModeChanged?.Invoke(true);
    }

    /// <summary>Programmatic exit from position edit mode. When <paramref name="save"/>
    /// is true the current OSD position is persisted as Custom (centre-anchored
    /// fractions), otherwise the pre-edit settings snapshot is restored.</summary>
    public void ExitPositionEditMode(bool save)
    {
        if (!_isEditMode) return;

        foreach (var overlay in _overlays)
        {
            overlay.PositionRequested -= OnOverlayClickRequested;
            overlay.SaveRequested -= OnOverlaySaveRequested;
            overlay.CancelRequested -= OnOverlayCancelRequested;
            overlay.Close();
        }
        _overlays.Clear();

        _isEditMode = false;

        if (save)
        {
            PersistCurrentPositionAsCustom();
        }
        else if (_preEditSnapshot is not null)
        {
            _settings.Save(_preEditSnapshot);
        }
        _preEditSnapshot = null;

        EditModeChanged?.Invoke(false);

        _cardHost.RequestShow(new ShowRequest(
            ShowReason.EditModeExit,
            null,
            TimeSpan.FromMilliseconds(Math.Max(_settings.Current.ShowDurationMs, 1500))));
    }

    private void OnOverlaySaveRequested() => ExitPositionEditMode(save: true);
    private void OnOverlayCancelRequested() => ExitPositionEditMode(save: false);

    // Overlay is the source of truth for snap decisions now (it already knows the
    // OSD rectangle from UpdateOsdRect). Whatever centre it emits is applied as-is;
    // mid-drag moves are unsnapped for smoothness, release-time and hotspot clicks
    // arrive already snapped to their target.
    private void OnOverlayClickRequested(Screen screen, Point absoluteCenter)
    {
        var (w, h) = MeasuredOsdSize();
        SetOsdCenteredAt(screen.WorkingArea, w, h, absoluteCenter.X, absoluteCenter.Y);
    }

    // The position editor's 3x3 hotspot grid is a Classic-only affordance (Task 8 disables
    // the editor in notch mode). It keeps its own margin rather than following the active
    // presentation's, whose notch value of 0 would collapse the outer hotspots onto the
    // screen edges.
    private const double EditorHotspotMarginDip = 96;

    // Nine snap targets = {EditorHotspotMarginDip, center, opposite-EditorHotspotMarginDip} on
    // each axis. Target values are OSD CENTRES (matching the persistence semantics), so a
    // corner hotspot's centre sits EditorHotspotMarginDip + w/2 from the working-area edge.
    private static (double cx, double cy) MaybeSnapCenter(Rect area, double w, double h, double cx, double cy)
    {
        // Alt = free placement, no magnet.
        if ((Keyboard.Modifiers & ModifierKeys.Alt) != 0) return (cx, cy);

        double[] cxTargets =
        {
            area.Left + EditorHotspotMarginDip + w / 2,       // left column centre
            area.Left + area.Width / 2,               // centre column centre
            area.Right - EditorHotspotMarginDip - w / 2,       // right column centre
        };
        double[] cyTargets =
        {
            area.Top + EditorHotspotMarginDip + h / 2,        // top row centre
            area.Top + area.Height / 2,               // middle row centre
            area.Bottom - EditorHotspotMarginDip - h / 2,      // bottom row centre
        };
        foreach (var tx in cxTargets)
            if (Math.Abs(cx - tx) < SnapThresholdDip) { cx = tx; break; }
        foreach (var ty in cyTargets)
            if (Math.Abs(cy - ty) < SnapThresholdDip) { cy = ty; break; }
        return (cx, cy);
    }

    private void SetOsdCenteredAt(Rect area, double w, double h, double cx, double cy)
    {
        var left = Math.Clamp(cx - w / 2, area.Left, Math.Max(area.Left, area.Right - w));
        var top = Math.Clamp(cy - h / 2, area.Top, Math.Max(area.Top, area.Bottom - h));
        Left = left;
        Top = top;
        ReassertTopmost();
        BroadcastOsdRect();
    }

    // Push the OSD's current absolute rectangle to every overlay so their drag hit-
    // test knows where the card sits. Called on Enter and after every reposition.
    private void BroadcastOsdRect()
    {
        var (w, h) = MeasuredOsdSize();
        var rect = new Rect(Left, Top, w, h);
        foreach (var overlay in _overlays) overlay.UpdateOsdRect(rect);
    }

    private (double w, double h) MeasuredOsdSize()
    {
        var w = _content.ActualWidth > 0 ? _content.ActualWidth : _content.DesiredSize.Width;
        var h = _content.ActualHeight > 0 ? _content.ActualHeight : _content.DesiredSize.Height;
        if (w == 0 || h == 0)
        {
            _content.Measure(new Size(double.PositiveInfinity, double.PositiveInfinity));
            _content.UpdateLayout();
            w = _content.DesiredSize.Width;
            h = _content.DesiredSize.Height;
        }
        return (w, h);
    }

    // Store the OSD's CENTRE as a fraction of the target monitor's working area (not
    // the top-left corner). Reposition() rebuilds Left/Top from those fractions using
    // the OSD's current size, so a content-size change (media card appearing) leaves
    // the OSD visually anchored on the same point.
    private void PersistCurrentPositionAsCustom()
    {
        var (w, h) = MeasuredOsdSize();
        var centerX = Left + w / 2;
        var centerY = Top + h / 2;
        var screen = Screen.FromPoint(new Point(centerX, centerY)) ?? Screen.PrimaryScreen;
        if (screen is null) return;
        var area = screen.WorkingArea;

        var m = _settings.Current.Clone();
        m.Position = OsdPosition.Custom;
        m.CustomPositionXPercent = Math.Clamp((centerX - area.Left) / area.Width, 0.0, 1.0);
        m.CustomPositionYPercent = Math.Clamp((centerY - area.Top) / area.Height, 0.0, 1.0);
        m.CustomPositionMonitorDeviceName = screen.DeviceName ?? string.Empty;
        _settings.Save(m);
    }
}
