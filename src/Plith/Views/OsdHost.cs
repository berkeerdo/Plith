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
using WpfScreenHelper;

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
        _hoverPoller = new NotchHoverPoller(Dispatcher);
        _hoverPoller.HoverChanged += OnNotchHoverChanged;
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

        _content = new OsdContent { DataContext = Shell };
        Content = _content;

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
        };

        // Seed the local accent mirror BEFORE the HwndSource is created (in CreateWindow
        // below) so the very first paint already uses the picked accent. Subsequent
        // updates come from ThemeService.ThemeApplied.
        RefreshAccentMirror();
        _theme.ThemeApplied += OnThemeApplied;

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
        // without waiting for the next pop. Card-level settings (colour thresholds, compact
        // mode) are owned by AudioCard and MediaCard and never travel through the shell.
        _settings.Changed += _ => Dispatcher.BeginInvoke(() =>
        {
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
    private bool WantsNotch =>
        _settings.Current.Presentation == PresentationMode.AmbientNotch && !_coversMonitor;

    private IOsdPresentation BuildPresentation() => WantsNotch
        ? new AmbientNotchPresentation(this, _content, () => _settings.Current.NotchStripHeightDip, _log)
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

        if (_presentation is AmbientNotchPresentation notch)
        {
            // A mode switch while the panel is open must settle fully closed, the same as
            // reaching rest normally does — otherwise the ambient row would survive a
            // rebuild that just re-parked the strip underneath it.
            _home.Close();
            Opacity = Math.Clamp(_settings.Current.OsdOpacityPercent, 50, 100) / 100.0;
            Reposition();               // measures content, which calls OnContentMeasured
            notch.PrepareShow();
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
        // No _coversMonitor check needed: BuildPresentation returns Classic while covered, so
        // a notch presentation existing at all already means nothing is covering the monitor.
        if (_presentation is AmbientNotchPresentation) _hoverPoller.Start();
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

        // Only the notch has a covered state to fall back from; Classic is already Classic.
        if (_settings.Current.Presentation != PresentationMode.AmbientNotch) return;

        ApplyPresentationMode();
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
    private void OnNotchHoverChanged(bool inside)
    {
        if (_isEditMode) return;
        if (!_settings.Current.HoverKeepAlive) return;
        if (_cardHost.Suppressor?.IsSuppressed == true) return;
        if (_presentation is not AmbientNotchPresentation) return;

        if (inside)
        {
            // Before ShowOsd, not after. ShowOsd measures the content to size the panel it
            // expands into, and the ambient row is part of that content — opening afterwards
            // would expand to a height computed without the row and clip it for one show.
            _home.Open();

            // Force the layout pass BEFORE the expansion starts. Open() adds the ambient card to
            // CardHost's collection, and the bound ItemsControl generates its container during a
            // measure/arrange — not at the moment the collection changes. Without this the
            // expansion begins against one target height and finishes against another as the new
            // container appears mid-flight, which is what makes the opening animation jump while
            // the closing one, whose content is already settled, stays smooth.
            _content.UpdateLayout();

            _hideTimer?.Stop();
            ShowOsd(TimeSpan.FromMilliseconds(_settings.Current.ShowDurationMs), fromHover: true);
        }
        else
        {
            // Leaving the resting rectangle is not leaving the OSD: it is only a few DIP tall,
            // so the ordinary way to leave it is by moving DOWN onto the open panel, which is
            // still squarely inside the window.
            //
            // This used to ask WPF (`if (IsMouseOver) return;`) and that was wrong, measured on a
            // running build: the OSD is a layered window with per-pixel alpha, Windows hit-tests
            // it against that alpha, and at rest the notch is a couple of opaque DIP in an
            // otherwise transparent window. The cursor that triggered the hover is over
            // transparent space, the panel opens beneath a now-stationary cursor, no further
            // WM_MOUSEMOVE is generated, and MouseEnter never fires — so IsMouseOver stayed false
            // for the panel's entire life and the hide timer took it away with the cursor on it.
            //
            // The poller has no such dependency: GetCursorPos answers regardless of alpha, of
            // message delivery, and of whether the user moved.
            if (_hoverPoller.IsCursorInPanel) return;
            if (_currentVisibleFor > TimeSpan.Zero && !_isFadingOut)
                RestartHideTimer(_currentVisibleFor);
        }

        // Skipped only by the IsMouseOver early return above, which is a deliberate no-op:
        // WantsHitTesting has not changed there (still descended, not re-parked), so
        // IsClickThrough is already correct and re-computing it would just repeat the same
        // value. Every other path through this method reaches here.
        IsClickThrough = !_presentation.WantsHitTesting;

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

    public void ShowOsd(TimeSpan visibleFor, bool fromHover = false)
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
