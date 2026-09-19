using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Input;
using System.Windows.Interop;
using System.Windows.Media;
using System.Windows.Media.Animation;
using System.Windows.Threading;
using Plith.Services.Shelf;
using Plith.Views.Presentation;

namespace Plith.DropCatcher.Shelf;

/// <summary>
/// The shelf page, as the window a person actually touches.
///
/// A SIBLING of <see cref="CatcherWindow"/>, not an extension of it, and the XAML beside this
/// file carries the long form of why. The short form: the catcher must never take activation
/// (it appears under a pointer that is already holding a file) and the shelf must always take
/// it (Esc, arrow keys and a visible selection have nothing to arrive at in a window that never
/// takes focus). Those are opposite answers, applied to the HWND once at SourceInitialized, so
/// one window cannot serve both.
///
/// Like the catcher, this window is never actually closed. It is hidden and shown again, because
/// WPF's <see cref="Window.Close"/> destroys the window for good and a destroyed window cannot be
/// reopened when the next OpenShelf arrives.
/// </summary>
public partial class ShelfWindow : Window
{
    private const int GWL_EXSTYLE = -20;
    private const int WS_EX_TOOLWINDOW = 0x00000080;

    private static readonly nint HWND_TOPMOST = -1;
    private const uint SWP_SHOWWINDOW = 0x0040;

    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool SetWindowPos(nint hWnd, nint hWndInsertAfter, int X, int Y, int cx, int cy, uint uFlags);

    [DllImport("user32.dll", SetLastError = true, EntryPoint = "GetWindowLongPtrW")]
    private static extern nint GetWindowLongPtr(nint hWnd, int nIndex);

    [DllImport("user32.dll", SetLastError = true, EntryPoint = "SetWindowLongPtrW")]
    private static extern nint SetWindowLongPtr(nint hWnd, int nIndex, nint dwNewLong);

    [DllImport("user32.dll")]
    private static extern nint GetForegroundWindow();

    /// <summary>
    /// How long the shape takes to grow into place. The same 220 ms the catcher uses, and for the
    /// same reason: both stand in for the notch, and a stand-in that arrives at a different speed
    /// reads as a second object rather than the same one continuing.
    /// </summary>
    private static readonly Duration GrowDuration = new(TimeSpan.FromMilliseconds(220));

    /// <summary>
    /// How long the shelf waits after the pointer leaves before it takes itself down.
    ///
    /// It is not zero, and that is the point. The pointer crosses outside the surface on the way
    /// to a tile at its edge, and on the way to anything this window later opens beside itself.
    /// Closing on the leave itself would make the shelf impossible to reach around its own edge.
    ///
    /// 500 ms was CHOSEN, not measured. What has been measured is only that it WORKS: driven with
    /// SetCursorPos, leaving and returning inside 250 ms keeps the shelf, and leaving and staying
    /// away takes it down once. Whether 500 ms is the right length is a different question and a
    /// scripted pointer cannot answer it, because the thing being judged is how a hand moving
    /// toward a tile at the edge feels. The hardware pass in docs/SHELF-VERIFICATION.md is what
    /// would correct it, and a person finding the shelf hard to leave, or too eager to go, is
    /// reading this number.
    /// </summary>
    private static readonly TimeSpan LeaveGrace = TimeSpan.FromMilliseconds(500);

    private readonly CatcherLog _log;
    private readonly ShelfModel _model = new();
    private readonly DispatcherTimer _leave;

    /// <summary>Why the shelf is waiting to go away, or null when it is not. Set when a dismissal
    /// arrives while a drag or a menu suspends it, and acted on by the next leave-timer tick. See
    /// <see cref="Dismiss"/> for the sequence that makes a deferral necessary rather than a
    /// nicety.</summary>
    private string? _pendingDismissal;

    /// <summary>Whether the shelf is currently up. Guards <see cref="CloseNow"/> so a second
    /// dismissal (an Esc landing in the same frame as a Deactivated, which does happen) cannot
    /// report the shelf closed twice and make Plith put the notch back twice.</summary>
    private bool _open;

    /// <summary>Set while a tile's context menu is up, and read by Dismiss: a context menu takes
    /// activation the same way losing focus to another window does, and without this a menu
    /// opening would dismiss the surface it belongs to out from under itself. Cleared on close,
    /// which matters as much as setting it: see the constructor's own comment on why it cannot
    /// be left set.</summary>
    private bool _menuOpen;

    // CS0649 is "never assigned to", and here that is the design rather than an oversight: this
    // field is read by Dismiss and written by a task that does not exist yet. Suppressed at the
    // one declaration only, and narrowly, so that the day a real never-assigned field appears
    // somewhere else in this file the compiler still says so.
#pragma warning disable CS0649

    /// <summary>Set by the drag out in Task 8. Declared here because Dismiss reads it, and a
    /// window that can only be dismissed correctly after a later task is a window that is wrong
    /// in between.</summary>
    private bool _dragInFlight;

#pragma warning restore CS0649

    /// <summary>
    /// Expansion progress, 0 = the notch's open frame, 1 = the shelf page. One value drives
    /// width, height, corner radius and content opacity, so none of them can drift out of step
    /// with the others: the same single-value rule NotchGeometry exists to enforce for the notch.
    /// </summary>
    private static readonly DependencyProperty ExpansionProperty = DependencyProperty.Register(
        nameof(Expansion), typeof(double), typeof(ShelfWindow),
        new PropertyMetadata(0.0, (d, e) => ((ShelfWindow)d).ApplyExpansion((double)e.NewValue)));

    private double Expansion
    {
        get => (double)GetValue(ExpansionProperty);
        set => SetValue(ExpansionProperty, value);
    }

    internal ShelfWindow(CatcherLog log)
    {
        _log = log;
        // Set once, here, rather than threaded through ShelfSurface's constructor or Apply: it
        // is the one place in the catcher that already holds the shared CatcherLog when a
        // ShelfSurface is about to start extracting icons. Left null (a no-op) in the render
        // harness and anywhere else that never runs this constructor.
        ShellIcons.Log = log.Info;
        InitializeComponent();
        SourceInitialized += OnSourceInitialized;

        // A palette before anything is painted, because ShelfSurface.Render resolves every brush
        // by key and a key nothing has defined throws. Plith sends the real one over the wire;
        // until then the shelf is drawn in the product's own dark palette rather than in WPF's
        // defaults, so a Palette message that never arrives shows as slightly wrong colours
        // rather than as an exception in the middle of a drop.
        //
        // The whole Apply, not just Page.Apply, and that distinction cost the probe its point.
        // Apply is also what paints the GROWING SHAPE's gradient, and the shape is what is on
        // screen for the first half of every growth. Applying the fallback to the page alone left
        // the shape at the flat XAML colour, so the page faded in over a background of a
        // different colour: exactly the shift this window paints its own gradient to avoid. No
        // palette sender exists yet, so that is the probe's own configuration, and the probe is
        // what the hardware pass judges.
        //
        // Apply ends in Render, so this also means an OpenShelf arriving before any Items message
        // shows the page's empty state rather than a blank panel. Plith sends the shelf whole on
        // every change, but a shelf with nothing on it has no stacks to send.
        Apply(BuiltInDark);

        _leave = new DispatcherTimer(DispatcherPriority.Normal, Dispatcher) { Interval = LeaveGrace };
        _leave.Tick += (_, _) =>
        {
            _leave.Stop();
            Dismiss("the pointer left and did not come back");
        };

        // Stopped only when there is nothing waiting. With a deferred dismissal pending, the
        // timer is the clock that re-evaluates it, and stopping it because the pointer came back
        // would strand the shelf in exactly the way the deferral exists to prevent.
        MouseEnter += (_, _) => { if (_pendingDismissal is null) _leave.Stop(); };
        MouseLeave += (_, _) => { _leave.Stop(); _leave.Start(); };
        Deactivated += (_, _) => Dismiss("another window took focus");

        // Getting activation back cancels a deferred dismissal, because the thing that asked for
        // it is no longer true. The case is a context menu: it takes activation, which raises
        // Deactivated, which defers; when the menu closes the shelf is foreground again and
        // closing it then would punish the person for having opened a menu on it. A drag out does
        // not reach here, since the shelf is not activated again at the end of one.
        Activated += (_, _) => _pendingDismissal = null;
        PreviewKeyDown += OnPreviewKeyDown;

        // EntryPressed: additive (Ctrl held) is Select's ordinary toggle. Non-additive is
        // DragPaths rather than a plain Select(path, false) - DragPaths carries the exact rule a
        // press needs here, leaving an already-selected tile's whole selection alone rather than
        // collapsing it to the one pressed, which is what lets a following drag still carry more
        // than one path. The paths DragPaths returns are not needed by this handler; only its
        // effect on Selection is, which Render then paints as the selection ring.
        Page.EntryPressed += (path, additive) =>
        {
            if (additive) _model.Select(path, true);
            else _model.DragPaths(path);
            Page.Render(_model);
        };

        // ClearRequested, NewStackRequested, RemoveRequested and RestackRequested all cross the
        // wire, and this window does not send them itself: App owns the one CatcherClient this
        // process has, the same reason CatcherWindow's FilesDropped and Withdrew are plain events
        // rather than direct sends. Bubbled through unchanged rather than translated to a
        // DropMessage here, so this file does not have to know the wire format to raise them.
        Page.ClearRequested += () => ClearShelfRequested?.Invoke();
        Page.NewStackRequested += () => NewStackRequested?.Invoke();
        Page.RemoveRequested += paths => RemoveItemsRequested?.Invoke(paths);
        Page.RestackRequested += (index, paths) => RestackRequested?.Invoke(index, paths);

        // OpenRequested and RevealRequested are the opposite: they never touch the wire at all,
        // because opening a file or showing it in the file manager is something THIS process
        // does on its own account, at the Medium integrity it already runs at. See ShelfActions'
        // own header comment for why that has to be true rather than being a shortcut, and why
        // Plith cannot do either of these itself.
        Page.OpenRequested += ShelfActions.Open;
        Page.RevealRequested += ShelfActions.ShowInFileManager;

        // A context menu takes activation exactly the way losing focus to another window does.
        // MenuOpenChanged, not ContextMenuOpening/Closing bubbling up from whichever tile opened
        // one: the first version used those, and a re-render (Render tears every tile out of
        // Columns and rebuilds them, and Plith re-sends the whole shelf after every mutating
        // verb) could destroy the tile that opened a menu before its Closing had anywhere left
        // to bubble through, leaving _menuOpen stuck true and the shelf undismissable forever.
        // MenuOpenChanged is raised by ShelfSurface itself, from the menu's own Opened/Closed
        // (popup content, not part of the tile's subtree) and force-closed inside Render before
        // a single tile is torn down, so it cannot go missing the same way. Cleared on close, not
        // left set: leaving it set would mean nothing could ever dismiss the shelf again once a
        // single menu had been opened.
        Page.MenuOpenChanged += open => _menuOpen = open;
    }

    /// <summary>
    /// Raised when the shelf has gone away, so Plith can put the notch back.
    ///
    /// Deliberately NOT called Closed. That name is taken by <see cref="Window.Closed"/>, and
    /// naming it the same would mean hiding a base member: any code holding a Window-typed
    /// reference and writing <c>window.Closed += ...</c> would silently bind WPF's event instead
    /// of this one, compile, and never fire the handler anyone meant. It would also be the wrong
    /// word. This window is hidden and shown again rather than closed, so Window.Closed never
    /// fires here at all, and a reader who saw both names would have to guess which one meant
    /// what.
    /// </summary>
    public event Action? Dismissed;

    /// <summary>The shelf's own controls asked to change what is on the shelf, and none of them
    /// are applied here: the model is a view of Plith's shelf, so every one of these is a
    /// REQUEST, and the answer is a fresh set of Items messages, not this window updating
    /// itself. App bridges these onto the wire as RemoveItems, ClearShelf, NewStack and Restack,
    /// since it is the one place that holds the client connection.</summary>
    public event Action? ClearShelfRequested;

    /// <inheritdoc cref="ClearShelfRequested"/>
    public event Action? NewStackRequested;

    /// <inheritdoc cref="ClearShelfRequested"/>
    public event Action<IReadOnlyList<string>>? RemoveItemsRequested;

    /// <inheritdoc cref="ClearShelfRequested"/>
    public event Action<int, IReadOnlyList<string>>? RestackRequested;

    private void OnSourceInitialized(object? sender, EventArgs e)
    {
        var handle = new WindowInteropHelper(this).Handle;

        // TOOLWINDOW keeps the shelf out of Alt+Tab, exactly as it keeps the catcher out.
        //
        // WS_EX_NOACTIVATE is NOT set, and its absence is the single most important line in this
        // file. The catcher sets it so that a click cannot pull focus away from a drag already in
        // progress. The shelf needs the opposite: with NOACTIVATE the window never becomes
        // foreground, so Esc never arrives and Deactivated never fires, leaving the mouse-leave
        // timer as the only way out of a window that is covering the top of the screen.
        var style = GetWindowLongPtr(handle, GWL_EXSTYLE);
        _ = SetWindowLongPtr(handle, GWL_EXSTYLE, style | WS_EX_TOOLWINDOW);
    }

    /// <summary>
    /// Physical screen pixels, applied without conversion, the same rectangle OpenShelf carries
    /// on the wire. WPF's Left/Top would route the same numbers through this process's own idea
    /// of the DPI scale, which is exactly the arithmetic the wire format exists to avoid having
    /// in two places.
    /// </summary>
    public void OpenAt(int x, int y, int width, int height)
    {
        // A second OpenShelf while the shelf is already up is a RE-ASSERTION, not a re-open: the
        // monitor changed, the DPI changed, or Plith simply restated where the shelf belongs. It
        // moves and resizes, and it does NOT grow again. Growing again would snap the shape back
        // to the notch's frame and the page back to invisible, so a message meaning "you are in
        // the right place" would read on screen as the shelf having been closed and reopened.
        var reasserting = IsVisible;

        // Show() first, and it is not optional. EnsureHandle() alone creates the HWND and
        // SWP_SHOWWINDOW alone makes it visible to the window manager, but WPF does not consider
        // the window shown: it builds no visual tree, so the page inside never loads and every
        // FindResource in ShelfSurface runs against a control that was never there. Measured on
        // CatcherWindow's first probe run, where the only symptom was MainWindowHandle staying 0.
        if (!reasserting)
        {
            // Collapsed BEFORE the window is shown, or the first frame is the whole page and the
            // growth animates out of something the person has already seen whole.
            BeginAnimation(ExpansionProperty, null);
            Expansion = 0;
            Show();
        }

        var handle = new WindowInteropHelper(this).Handle;

        // No SWP_NOACTIVATE here, unlike the catcher: this window wants the activation.
        _ = SetWindowPos(handle, HWND_TOPMOST, x, y, width, height, SWP_SHOWWINDOW);

        // Forced, rather than waiting for the next layout pass. ApplyExpansion reads ActualWidth
        // and ActualHeight to know where the growth ends up, and the animation's first tick can
        // run before the WM_SIZE that SetWindowPos just sent has been laid out. Without this the
        // first frames grow toward the previous size instead of this one.
        UpdateLayout();

        _open = true;
        _leave.Stop();

        Activate();
        Keyboard.Focus(this);

        if (reasserting)
        {
            // Re-applied at full expansion rather than animated, because the rectangle may have
            // changed and both the shape and the page are sized from it. The property is already
            // held at 1 by the finished animation, so assigning it would be ignored; calling the
            // handler directly is the one path that re-reads the new ActualWidth/ActualHeight.
            ApplyExpansion(1);
        }
        else
        {
            BeginAnimation(ExpansionProperty, new DoubleAnimation(0, 1, GrowDuration)
            {
                EasingFunction = new CubicEase { EasingMode = EasingMode.EaseOut },
            });
        }

        // Logged rather than assumed. A process that is not already the foreground process is not
        // always allowed to become one (Windows' foreground lock), and when Activate() loses that
        // race the window is on screen, topmost and unfocused: Esc does nothing and Deactivated
        // never fires, because the window was never activated. That failure looks identical on
        // screen to a working shelf, so it is recorded here rather than left to be reported as
        // "Esc does not close it".
        var foreground = GetForegroundWindow() == handle;
        _log.Info($"Shelf {(reasserting ? "re-asserted" : "opened")} at {x},{y} {width}x{height}. " +
                  $"handle=0x{handle:X}, foreground={foreground}");
    }

    /// <summary>Repaints the shelf in the theme Plith resolved. Safe at any time: the page is
    /// re-rendered afterwards, so brushes already handed to existing tiles are replaced rather
    /// than left behind at the old colours.</summary>
    public void Apply(ShelfPalette palette)
    {
        Page.Apply(palette);

        // The growing shape is painted by this window, not by the page inside it, because for
        // most of the growth the page is fully transparent and there would otherwise be nothing
        // on screen at all. It therefore needs the same gradient the page's own surface uses, or
        // the last frames of the growth would visibly change colour as the page faded in over a
        // different background.
        var background = new LinearGradientBrush(
            palette.SurfaceStart, palette.SurfaceEnd, new Point(0, 0), new Point(0, 1));
        background.Freeze();
        Shape.Background = background;

        Page.Render(_model);
    }

    /// <summary>One stack of the shelf, placed at the index the message carries. The model, not
    /// this window, decides whether the message belongs to the delivery currently being
    /// assembled: see ShelfModel.SetStack for why arrival order cannot be trusted.</summary>
    public void SetStack(int index, int total, IReadOnlyList<string> paths)
    {
        _model.SetStack(index, total, paths);
        Page.Render(_model);
    }

    /// <summary>
    /// Takes the shelf down. Hide(), not Close(): a closed WPF window cannot be shown again, and
    /// the next OpenShelf would have nothing to open.
    ///
    /// Hide() rather than SWP_HIDEWINDOW for the reason the catcher uses it too: WPF's own idea
    /// of visibility has to stay in step with the window manager's, or the next OpenAt skips
    /// Show() and lands back in the no-visual-tree bug above.
    /// </summary>
    public void CloseNow()
    {
        _leave.Stop();
        if (!_open) return;
        _open = false;

        Hide();

        // Cleared rather than left at 1. BeginAnimation(prop, null) removes the clock WITHOUT
        // raising Completed, which is a trap this codebase has been caught by repeatedly, but
        // here nothing is waiting on a callback and leaving the animation attached would hold the
        // property at its final value and make the next OpenAt start from the finished page.
        BeginAnimation(ExpansionProperty, null);
        Expansion = 0;

        // The selection belongs to a shelf that is on screen. Kept across a close, the next
        // OpenShelf would come up with tiles ringed from a session the person has already ended.
        //
        // Rendered as well, and the render is the half that does the work. ShelfSurface paints
        // the ring at Render time from the selection it was handed, and OpenAt does not render,
        // so a shelf re-opened without an intervening Items message would come up still showing
        // the old rings no matter what the model said.
        _model.ClearSelection();
        Page.Render(_model);

        _pendingDismissal = null;
        Dismissed?.Invoke();
    }

    /// <summary>
    /// Every way the shelf goes away, and the two states that suspend all of them.
    ///
    /// A drag in flight must not dismiss the surface: the person is holding a file over another
    /// application, the shelf has lost activation by definition, and closing under them would
    /// cancel the gesture they are in the middle of. A context menu takes activation too, for
    /// the same reason and with the same answer.
    ///
    /// Suppressed means DEFERRED, never cancelled, and that distinction is the whole of the
    /// second half of this method. The failing sequence, written down so it cannot be simplified
    /// back out: once Task 8 sets _dragInFlight, pressing a tile, dragging off the shelf and
    /// releasing over another application consumes BOTH remaining dismissals. The leave timer
    /// fires, is suppressed, and nothing re-arms it; the Deactivated fires, is suppressed, and no
    /// second one can ever follow, because the window is not active any more. The shelf is then
    /// stranded on screen with no way off it. So a suppressed dismissal is remembered and the
    /// leave timer is re-armed as the clock that re-evaluates it: a retry needs no cooperation
    /// from the task that sets the flag, which is what makes it correct before that task exists.
    /// </summary>
    private void Dismiss(string why)
    {
        // CloseNow guards on this too, and it has to, because it is public. The check is repeated
        // here, and FIRST, for two reasons. The log stays honest: Hide() deactivates the window,
        // so an Esc is always followed a millisecond later by a Deactivated, and without this the
        // log claimed the shelf closed twice for two different reasons (measured on the first
        // probe run). And a dismissal arriving after the shelf is already down must not leave a
        // retry clock running against a window nobody can see.
        if (!_open) return;

        if (_dragInFlight || _menuOpen)
        {
            // The FIRST reason is kept, not the latest. What the person did to dismiss the shelf
            // is the interesting line in the log; the retries after it are this method talking to
            // itself.
            if (_pendingDismissal is null)
            {
                _pendingDismissal = why;
                _log.Info($"Shelf dismissal deferred ({why}): a drag or a menu is in flight.");
            }

            _leave.Stop();
            _leave.Start();
            return;
        }

        _log.Info($"Shelf closing: {_pendingDismissal ?? why}.");
        CloseNow();
    }

    private void OnPreviewKeyDown(object sender, KeyEventArgs e)
    {
        if (e.Key != Key.Escape) return;

        e.Handled = true;
        Dismiss("Esc");
    }

    /// <summary>
    /// The shape at a given expansion, and the page held still inside it.
    ///
    /// The growth starts at the notch's OPEN FRAME rather than at its resting strip, because the
    /// open frame is what was on screen a moment ago: a click on the open notch is what asks for
    /// the shelf, so the shelf continues the shape the person is already looking at.
    /// </summary>
    private void ApplyExpansion(double t)
    {
        // ActualWidth/Height rather than the XAML values: the window has just been sized to the
        // rectangle Plith handed over, which is where the growth has to end up.
        var open = new Size(ActualWidth, ActualHeight);
        var from = NotchGeometry.OpenFrameDip;
        var size = NotchGeometry.SurfaceSize(from.Width, from.Height, open, t);

        Shape.Width = size.Width;
        Shape.Height = size.Height;
        var radius = NotchGeometry.SurfaceRadius(t, size.Height);
        Shape.CornerRadius = new CornerRadius(0, 0, radius, radius);

        // The page is pinned at the FINAL size for the whole growth and clipped by the shape
        // around it, rather than being laid out into whatever the shape currently measures. Laid
        // out every frame, its tile columns would reflow from a 356 DIP frame to a 384 DIP one
        // while fading in, which reads as a window being resized rather than a shape opening. The
        // same Math.Max clamp SurfaceSize applies is repeated here so the page cannot disagree
        // with the shape about where the growth ends.
        Page.Width = Math.Max(open.Width, from.Width);
        Page.Height = Math.Max(open.Height, from.Height);
        Page.Opacity = NotchGeometry.ContentOpacity(t);
    }

    /// <summary>
    /// The colours used until a Palette message arrives, taken from Resources/OsdPalette.Dark.xaml
    /// rather than invented, so the fallback is the product's own dark theme rather than a second
    /// palette nothing else in the product uses.
    ///
    /// SelectionRing is the accent unchanged, which is what ContrastInk.RingOn returns for this
    /// pair: emerald on a near-black surface clears the 3:1 a non-text stroke needs several times
    /// over. The derivation itself stays on Plith's side, as ShelfPaletteWire says it must.
    /// </summary>
    private static readonly ShelfPalette BuiltInDark = new(
        SurfaceStart: Color.FromArgb(0xF0, 0x18, 0x18, 0x18),
        SurfaceEnd: Color.FromArgb(0xF0, 0x11, 0x11, 0x11),
        Ink: Color.FromRgb(0xF2, 0xF5, 0xF8),
        InkMuted: Color.FromRgb(0x9A, 0xA6, 0xB2),
        Track: Color.FromRgb(0x2A, 0x32, 0x3C),
        Accent: Color.FromRgb(0x4A, 0xD6, 0x95),
        SelectionRing: Color.FromRgb(0x4A, 0xD6, 0x95),
        IsDark: true);
}
