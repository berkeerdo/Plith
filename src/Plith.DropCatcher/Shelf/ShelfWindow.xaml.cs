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
    /// </summary>
    private static readonly TimeSpan LeaveGrace = TimeSpan.FromMilliseconds(500);

    private readonly CatcherLog _log;
    private readonly ShelfModel _model = new();
    private readonly DispatcherTimer _leave;

    /// <summary>Whether the shelf is currently up. Guards <see cref="CloseNow"/> so a second
    /// dismissal (an Esc landing in the same frame as a Deactivated, which does happen) cannot
    /// report the shelf closed twice and make Plith put the notch back twice.</summary>
    private bool _open;

    // CS0649 is "never assigned to", and here that is the design rather than an oversight: both
    // fields are read by Dismiss and written by tasks that do not exist yet. Suppressed at the
    // two declarations only, and narrowly, so that the day a real never-assigned field appears
    // somewhere else in this file the compiler still says so.
#pragma warning disable CS0649

    /// <summary>Set by the drag out in Task 8. Declared here because Dismiss reads it, and a
    /// window that can only be dismissed correctly after a later task is a window that is wrong
    /// in between.</summary>
    private bool _dragInFlight;

    /// <summary>Set by the context menu in Task 7, same reason.</summary>
    private bool _menuOpen;

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
        InitializeComponent();
        SourceInitialized += OnSourceInitialized;

        // A palette before anything is painted, because ShelfSurface.Render resolves every brush
        // by key and a key nothing has defined throws. Plith sends the real one over the wire;
        // until then the shelf is drawn in the product's own dark palette rather than in WPF's
        // defaults, so a Palette message that never arrives shows as slightly wrong colours
        // rather than as an exception in the middle of a drop.
        Page.Apply(BuiltInDark);

        // Rendered once here as well, so an OpenShelf that arrives before any Items message shows
        // the page's empty state rather than a blank panel. Plith sends the shelf whole on every
        // change, but a shelf with nothing on it has no stacks to send.
        Page.Render(_model);

        _leave = new DispatcherTimer(DispatcherPriority.Normal, Dispatcher) { Interval = LeaveGrace };
        _leave.Tick += (_, _) =>
        {
            _leave.Stop();
            Dismiss("the pointer left and did not come back");
        };

        MouseEnter += (_, _) => _leave.Stop();
        MouseLeave += (_, _) => { _leave.Stop(); _leave.Start(); };
        Deactivated += (_, _) => Dismiss("another window took focus");
        PreviewKeyDown += OnPreviewKeyDown;

        // ShelfSurface's three events (EntryPressed, ClearRequested, NewStackRequested) are
        // deliberately NOT subscribed here. Every one of them answers with a message back over
        // the wire (RemoveItems, ClearShelf, NewStack, Restack), and the send side of this
        // process belongs to a later task. Subscribing now would mean either a handler that
        // silently does nothing, or a second subscriber that the later task has to notice and
        // remove rather than simply add to.
    }

    /// <summary>
    /// Raised when the shelf has gone away, so Plith can put the notch back.
    ///
    /// This deliberately HIDES <see cref="Window.Closed"/>, which is unusual enough to justify:
    /// this window is hidden and reshown rather than closed, so the base event never fires and
    /// there is nothing here to lose; and the base signature would force every subscriber to
    /// carry a sender and an EventArgs that neither end of this wire has any use for.
    /// </summary>
    public new event Action? Closed;

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
        // Show() first, and it is not optional. EnsureHandle() alone creates the HWND and
        // SWP_SHOWWINDOW alone makes it visible to the window manager, but WPF does not consider
        // the window shown: it builds no visual tree, so the page inside never loads and every
        // FindResource in ShelfSurface runs against a control that was never there. Measured on
        // CatcherWindow's first probe run, where the only symptom was MainWindowHandle staying 0.
        if (!IsVisible)
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

        BeginAnimation(ExpansionProperty, new DoubleAnimation(0, 1, GrowDuration)
        {
            EasingFunction = new CubicEase { EasingMode = EasingMode.EaseOut },
        });

        // Logged rather than assumed. A process that is not already the foreground process is not
        // always allowed to become one (Windows' foreground lock), and when Activate() loses that
        // race the window is on screen, topmost and unfocused: Esc does nothing and Deactivated
        // never fires, because the window was never activated. That failure looks identical on
        // screen to a working shelf, so it is recorded here rather than left to be reported as
        // "Esc does not close it".
        var foreground = GetForegroundWindow() == handle;
        _log.Info($"Shelf opened at {x},{y} {width}x{height}. handle=0x{handle:X}, foreground={foreground}");
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
        _model.ClearSelection();

        Closed?.Invoke();
    }

    /// <summary>
    /// Every way the shelf goes away, and the two states that suspend all of them.
    ///
    /// A drag in flight must not dismiss the surface: the person is holding a file over another
    /// application, the shelf has lost activation by definition, and closing under them would
    /// cancel the gesture they are in the middle of. A context menu takes activation too, for
    /// the same reason and with the same answer.
    /// </summary>
    private void Dismiss(string why)
    {
        if (_dragInFlight || _menuOpen) return;

        // CloseNow guards on this too, and it has to, because it is public. The check is repeated
        // here so the LOG stays honest: Hide() deactivates the window, so an Esc is always
        // followed a millisecond later by a Deactivated, and without this the log claimed the
        // shelf closed twice for two different reasons. Measured on the first probe run.
        if (!_open) return;

        _log.Info($"Shelf closing: {why}.");
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
