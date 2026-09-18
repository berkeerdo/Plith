using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Interop;
using System.Windows.Media;
using System.Windows.Media.Animation;
using System.Windows.Threading;
using Plith.Views.Presentation;

namespace Plith.DropCatcher;

/// <summary>
/// The window that stands in for the notch while a drag is in flight.
///
/// It is an ordinary Medium-integrity window, and that is its entire qualification: Plith's own
/// window has a registered drop target and still receives neither a DragEnter nor a WM_DROPFILES,
/// because UIAccess puts Plith at High integrity and UIPI refuses the cross-integrity COM call
/// Explorer makes to start the drag.
/// </summary>
public partial class CatcherWindow : Window
{
    private const int GWL_EXSTYLE = -20;
    private const int WS_EX_TOOLWINDOW = 0x00000080;
    private const int WS_EX_NOACTIVATE = 0x08000000;

    private static readonly nint HWND_TOPMOST = -1;
    private const uint SWP_NOACTIVATE = 0x0010;
    private const uint SWP_SHOWWINDOW = 0x0040;
    private const uint SWP_HIDEWINDOW = 0x0080;

    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool SetWindowPos(nint hWnd, nint hWndInsertAfter, int X, int Y, int cx, int cy, uint uFlags);

    [DllImport("user32.dll", SetLastError = true, EntryPoint = "GetWindowLongPtrW")]
    private static extern nint GetWindowLongPtr(nint hWnd, int nIndex);

    [DllImport("user32.dll", SetLastError = true, EntryPoint = "SetWindowLongPtrW")]
    private static extern nint SetWindowLongPtr(nint hWnd, int nIndex, nint dwNewLong);

    private readonly CatcherLog _log;

    /// <summary>
    /// How long the catcher will stand in the notch's place before deciding no file drag is
    /// coming and withdrawing.
    ///
    /// It exists because nothing readable from the cursor distinguishes a file being carried to
    /// the top of the screen from a window being dragged there to maximise, so Plith hands over
    /// for both. A real drag raises DragEnter within a frame or two of this window appearing —
    /// it is already under the cursor — so anything past this is the other case.
    /// </summary>
    private static readonly TimeSpan WithdrawAfter = TimeSpan.FromMilliseconds(450);

    private readonly DispatcherTimer _withdraw;
    private bool _sawDrag;

    /// <summary>
    /// How long the shape takes to grow into place. Matched to the notch's own open rather than
    /// chosen: the catcher is standing in for it, and a stand-in that arrives at a different
    /// speed reads as a second object rather than the same one continuing.
    /// </summary>
    private static readonly Duration GrowDuration = new(TimeSpan.FromMilliseconds(220));

    /// <summary>
    /// Expansion progress, 0 = the resting strip, 1 = the open panel. One value drives width,
    /// height, corner radius and content opacity, so none of them can drift out of step with the
    /// others — the same single-value rule NotchGeometry exists to enforce for the notch.
    /// </summary>
    private static readonly DependencyProperty ExpansionProperty = DependencyProperty.Register(
        nameof(Expansion), typeof(double), typeof(CatcherWindow),
        new PropertyMetadata(0.0, (d, e) => ((CatcherWindow)d).ApplyExpansion((double)e.NewValue)));

    private double Expansion
    {
        get => (double)GetValue(ExpansionProperty);
        set => SetValue(ExpansionProperty, value);
    }

    internal CatcherWindow(CatcherLog log)
    {
        _log = log;
        InitializeComponent();
        SourceInitialized += OnSourceInitialized;

        _withdraw = new DispatcherTimer(DispatcherPriority.Normal, Dispatcher) { Interval = WithdrawAfter };
        _withdraw.Tick += (_, _) =>
        {
            _withdraw.Stop();
            if (_sawDrag) return;

            _log.Info("No drag arrived; withdrawing.");
            HideNow();
            Withdrew?.Invoke();
        };
    }

    /// <summary>Raised when the catcher took itself down without a drop, so Plith can put the
    /// notch back rather than leaving it hidden for the rest of the gesture.</summary>
    public event Action? Withdrew;

    /// <summary>Raised on the UI thread with whatever the shell handed over. The paths are not
    /// checked here — Plith stats them, because Plith is the side that has to care.</summary>
    public event Action<IReadOnlyList<string>>? FilesDropped;

    private void OnSourceInitialized(object? sender, EventArgs e)
    {
        var handle = new WindowInteropHelper(this).Handle;

        // TOOLWINDOW keeps it out of Alt+Tab; NOACTIVATE keeps a click on it from pulling focus
        // away from whatever the person was doing. Neither affects the drop.
        var style = GetWindowLongPtr(handle, GWL_EXSTYLE);
        _ = SetWindowLongPtr(handle, GWL_EXSTYLE, style | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE);

    }

    /// <summary>
    /// Physical screen pixels, applied without conversion. WPF's Left/Top would route the same
    /// numbers through this process's own idea of the DPI scale, which is exactly the arithmetic
    /// the wire format exists to avoid duplicating.
    /// </summary>
    public void ShowAt(int x, int y, int width, int height)
    {
        // Show() first, and it is not optional: EnsureHandle() alone creates the HWND and
        // SWP_SHOWWINDOW alone makes it visible to the window manager, but WPF does not consider
        // the window shown — it builds no visual tree and registers no OLE drop target. The
        // result is a window that exists, sits in the right rectangle, draws nothing, and
        // silently refuses every drop. Measured on the first probe run, where the only symptom
        // was MainWindowHandle staying 0.
        if (!IsVisible)
        {
            // Collapsed BEFORE the window is shown, or the first frame is the open panel and the
            // growth animates out of something the person already saw whole.
            BeginAnimation(ExpansionProperty, null);
            Expansion = 0;
            Show();
        }

        _sawDrag = false;
        _withdraw.Stop();
        _withdraw.Start();

        var handle = new WindowInteropHelper(this).Handle;
        _ = SetWindowPos(handle, HWND_TOPMOST, x, y, width, height, SWP_NOACTIVATE | SWP_SHOWWINDOW);

        BeginAnimation(ExpansionProperty, new DoubleAnimation(0, 1, GrowDuration)
        {
            EasingFunction = new CubicEase { EasingMode = EasingMode.EaseOut },
        });

        _log.Info($"Shown at {x},{y} {width}x{height}. AllowDrop={AllowDrop}, handle=0x{handle:X}");
    }

    /// <summary>
    /// The shape at a given expansion. Width, height and radius all read from NotchGeometry —
    /// the notch's own file, linked into this project — so the stand-in grows the curve the notch
    /// grows rather than an approximation of it.
    /// </summary>
    private void ApplyExpansion(double t)
    {
        // ActualWidth/Height rather than the XAML values: the window has just been sized to the
        // rectangle Plith handed over, which is where the open shape has to end up.
        var open = new Size(ActualWidth, ActualHeight);
        var size = NotchGeometry.SurfaceSize(NotchGeometry.CollapsedWidthDip, CollapsedHeightDip, open, t);

        Shape.Width = size.Width;
        Shape.Height = size.Height;
        Shape.CornerRadius = new CornerRadius(0, 0,
            NotchGeometry.SurfaceRadius(t, size.Height), NotchGeometry.SurfaceRadius(t, size.Height));
        Body.Opacity = NotchGeometry.ContentOpacity(t);
    }

    /// <summary>
    /// Where the shape starts from. The notch's own resting height is a user setting that goes
    /// down to 2 DIP, and the catcher is not told it — but this is a transition's first frame
    /// rather than a state anyone looks at, and a few DIP either way is invisible at 220 ms.
    /// </summary>
    private const double CollapsedHeightDip = 6;

    public void HideNow()
    {
        _withdraw.Stop();
        if (!IsVisible) return;

        // Hide() rather than SWP_HIDEWINDOW, so WPF's own idea of visibility stays in step with
        // the window manager's. Out of step, the next ShowAt would skip Show() and land back in
        // the bug above.
        Hide();

        // Cleared rather than left at 1. BeginAnimation(prop, null) removes the clock WITHOUT
        // raising Completed, which is a trap this codebase has been caught by four times — but
        // here nothing is waiting on a callback, and leaving the animation attached would hold
        // the property at its final value and make the next ShowAt start from the open panel.
        BeginAnimation(ExpansionProperty, null);
        Expansion = 0;

        _log.Info("Hidden.");
    }

    protected override void OnDragEnter(DragEventArgs e)
    {
        base.OnDragEnter(e);
        _sawDrag = true;
        _withdraw.Stop();
        _log.Info($"DragEnter. FileDrop present: {e.Data.GetDataPresent(DataFormats.FileDrop)}");
        e.Effects = Effects(e);
        e.Handled = true;
    }

    protected override void OnDragOver(DragEventArgs e)
    {
        base.OnDragOver(e);
        e.Effects = Effects(e);
        e.Handled = true;
    }

    protected override void OnDrop(DragEventArgs e)
    {
        base.OnDrop(e);
        e.Handled = true;

        var paths = e.Data.GetData(DataFormats.FileDrop) as string[] ?? [];
        _log.Info($"DROP: {paths.Length} path(s): {string.Join(" | ", paths)}");

        HideNow();
        if (paths.Length > 0) FilesDropped?.Invoke(paths);
    }

    /// <summary>
    /// Copy, not Move — the shelf stages a reference to a file, it does not take the file away
    /// from where it was. Move would let a drag onto the notch delete the original.
    /// </summary>
    private static DragDropEffects Effects(DragEventArgs e)
        => e.Data.GetDataPresent(DataFormats.FileDrop) ? DragDropEffects.Copy : DragDropEffects.None;
}
